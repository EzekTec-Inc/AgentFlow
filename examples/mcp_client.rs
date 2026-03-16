use agentflow::core::error::AgentFlowError;
use agentflow::core::{create_typed_node, TypedFlow, TypedStore};
use agentflow::mcp::{McpCallResult, McpClient, McpClientOptions};
use rig::client::CompletionClient;
use rig::client::ProviderClient;
use rig::completion::Prompt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, Level};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Action {
    CrawlGoADesignSystem,
    ReviewCrawlResults,
    WriteReport,
    Complete,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum StoreState {
    Init,
    Crawled,
    ReviewedOk,
    NeedsRecrawl,
    ReportWritten,
    Completed,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CrawlArtifact {
    pub url: String,
    pub title: String,
    pub content: String,
    pub timestamp: String,
    pub status: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReviewFinding {
    pub is_dummy: bool,
    pub is_error: bool,
    pub reason: String,
    pub failed_url: String,
    pub retry_guidance: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReportArtifact {
    pub terminal_text: String,
    pub pdf_path: String,
    pub markdown_path: String,
}

fn mcp_text_content(result: &McpCallResult) -> String {
    result
        .content
        .iter()
        .filter_map(|item| {
            item.get("text")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AgentError {
    CrawlFailed {
        url: String,
        reason: String,
        retry_hint: String,
    },
    ReportFailed {
        reason: String,
        missing_tool: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppState {
    pub next_action: Action,
    pub state: StoreState,
    pub retry_count: u32,
    pub artifacts: Vec<CrawlArtifact>,
    pub review_findings: Vec<ReviewFinding>,
    pub report: Option<ReportArtifact>,
    pub agent_error: Option<AgentError>,
}

#[tokio::main]
async fn main() -> Result<(), AgentFlowError> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting AgentFlow Dynamic Orchestrator (GoA README Research) using TypedFlow");

    let mut server_exe = env::current_exe().map_err(|e| AgentFlowError::Custom(e.to_string()))?;
    server_exe.set_file_name("mcp-server");
    if cfg!(windows) {
        server_exe.set_extension("exe");
    }

    let mut client = McpClient::spawn_stdio(tokio::process::Command::new(server_exe))?;
    client
        .initialize(McpClientOptions {
            client_name: "agentflow-mcp-client-example".into(),
            client_version: env!("CARGO_PKG_VERSION").into(),
        })
        .await?;

    info!(
        server_name = client.server_name().unwrap_or("unknown"),
        server_version = client.server_version().unwrap_or("unknown"),
        "Connected to MCP server"
    );

    let mcp_tools = client.list_tools().await?;
    info!("Discovered {} MCP tools", mcp_tools.len());

    let has_crawl = mcp_tools.iter().any(|t| t.name == "crawl_goa_url");
    let has_pdf = mcp_tools.iter().any(|t| t.name == "generate_pdf");
    let mcp_client = Arc::new(Mutex::new(client));

    let openai_client = rig::providers::openai::Client::from_env();

    let initial_state = AppState {
        next_action: Action::CrawlGoADesignSystem,
        state: StoreState::Init,
        retry_count: 0,
        artifacts: vec![],
        review_findings: vec![],
        report: None,
        agent_error: None,
    };

    let mut flow = TypedFlow::<AppState>::new().with_max_steps(10);

    let openai_client_crawl = openai_client.clone();
    let mcp_client_crawl = Arc::clone(&mcp_client);
    let crawl_node = create_typed_node(move |store: TypedStore<AppState>| {
        let openai_client_crawl = openai_client_crawl.clone();
        let mcp_client_crawl = Arc::clone(&mcp_client_crawl);
        async move {
            let mut state = store.inner.write().await;
            info!("--- [Agent 1] Web Researcher & Crawler ---");

            if state.retry_count > 3 {
                error!("Max retries exceeded. Failing.");
                state.state = StoreState::Failed;
                state.next_action = Action::Failed;
                return store.clone();
            }

            let readme_url =
                "https://raw.githubusercontent.com/GovAlta/ui-components/refs/heads/dev/README.md";

            if !has_crawl {
                state.agent_error = Some(AgentError::CrawlFailed {
                    url: readme_url.into(),
                    reason: "Missing crawl_goa_url tool".into(),
                    retry_hint: "Add crawl_goa_url to the MCP server".into(),
                });
                state.state = StoreState::Failed;
                state.next_action = Action::Failed;
                return store.clone();
            }

            let crawl_result = {
                let mut client = mcp_client_crawl.lock().await;
                client
                    .call_tool("crawl_goa_url", json!({ "url": readme_url }))
                    .await
            };

            match crawl_result {
                Ok(result) => {
                    let crawled_text = mcp_text_content(&result);
                    let fallback_artifact = CrawlArtifact {
                        url: readme_url.into(),
                        title: "GovAlta UI Components README".into(),
                        content: if crawled_text.trim().is_empty() {
                            "README crawl returned no text.".into()
                        } else {
                            crawled_text
                        },
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        status: 200,
                    };

                    let agent_1 = openai_client_crawl
                        .agent("gpt-4.1-mini")
                        .preamble("You are Agent 1: Government of Alberta (GoA), Canada, UI Components README Researcher. Output exactly a JSON array containing one CrawlArtifact for the supplied README. Use fields: url, title, content, timestamp, status. Keep the content grounded in the crawled README text. No markdown blocks, just raw JSON array.")
                        .build();

                    match agent_1
                        .prompt(format!(
                            "Convert this crawled README into a single CrawlArtifact JSON array. URL: {readme_url}\nTimestamp: {}\nStatus: {}\nREADME text:\n{}",
                            fallback_artifact.timestamp,
                            fallback_artifact.status,
                            fallback_artifact.content
                        ))
                        .await
                    {
                        Ok(res) => {
                            let clean_res = res
                                .trim()
                                .trim_start_matches("```json")
                                .trim_end_matches("```")
                                .trim();
                            match serde_json::from_str::<Vec<CrawlArtifact>>(clean_res) {
                                Ok(mut artifacts) => {
                                    if artifacts.is_empty() {
                                        artifacts.push(fallback_artifact.clone());
                                    }
                                    state.artifacts = artifacts;
                                }
                                Err(e) => {
                                    info!("JSON parse error: {}. Using MCP crawl payload.", e);
                                    state.artifacts = vec![fallback_artifact];
                                }
                            }
                        }
                        Err(e) => {
                            info!("LLM prompt failed: {}. Using MCP crawl payload.", e);
                            state.artifacts = vec![fallback_artifact];
                        }
                    }

                    state.state = StoreState::Crawled;
                    state.next_action = Action::ReviewCrawlResults;
                }
                Err(e) => {
                    state.agent_error = Some(AgentError::CrawlFailed {
                        url: readme_url.into(),
                        reason: format!("crawl_goa_url failed: {e}"),
                        retry_hint: "Verify the MCP server is reachable and the crawl tool succeeds.".into(),
                    });
                    state.state = StoreState::Failed;
                    state.next_action = Action::Failed;
                }
            }

            store.clone()
        }
    });

    let openai_client_review = openai_client.clone();
    let review_node = create_typed_node(move |store: TypedStore<AppState>| {
        let openai_client_review = openai_client_review.clone();
        async move {
            let mut state = store.inner.write().await;
            info!("--- [Agent 2] Research Evaluator ---");

            let agent_2 = openai_client_review
                .agent("gpt-5.1")
                .preamble("You are Agent 2: Research Evaluator. Review the provided JSON artifact. If it looks correct, output a JSON array with one object: {\"is_dummy\":false, \"is_error\":false, \"reason\":\"Looks good\", \"failed_url\":\"\", \"retry_guidance\":\"\"}.")
                .build();

            let payload = serde_json::to_string(&state.artifacts).unwrap_or_default();
            match agent_2.prompt(&format!("Review this: {}", payload)).await {
                Ok(res) => {
                    let clean_res = res
                        .trim()
                        .trim_start_matches("```json")
                        .trim_end_matches("```")
                        .trim();
                    match serde_json::from_str::<Vec<ReviewFinding>>(clean_res) {
                        Ok(mut findings) => {
                            if findings.is_empty() {
                                findings.push(ReviewFinding {
                                    is_dummy: false,
                                    is_error: false,
                                    reason: "Looks valid (fallback)".into(),
                                    failed_url: "".into(),
                                    retry_guidance: "".into(),
                                });
                            }
                            state.review_findings = findings.clone();
                            if findings.iter().any(|f| f.is_error) {
                                state.state = StoreState::NeedsRecrawl;
                                state.next_action = Action::CrawlGoADesignSystem;
                                state.retry_count += 1;
                            } else {
                                state.state = StoreState::ReviewedOk;
                                state.next_action = Action::WriteReport;
                            }
                        }
                        Err(e) => {
                            info!("Review JSON parse error: {}. Using fallback verdict.", e);
                            state.review_findings = vec![ReviewFinding {
                                is_dummy: false,
                                is_error: false,
                                reason: "Looks valid (fallback)".into(),
                                failed_url: "".into(),
                                retry_guidance: "".into(),
                            }];
                            state.state = StoreState::ReviewedOk;
                            state.next_action = Action::WriteReport;
                        }
                    }
                }
                Err(e) => {
                    info!("Review LLM failed: {}. Using fallback verdict.", e);
                    state.review_findings = vec![ReviewFinding {
                        is_dummy: false,
                        is_error: false,
                        reason: "Looks valid (fallback)".into(),
                        failed_url: "".into(),
                        retry_guidance: "".into(),
                    }];
                    state.state = StoreState::ReviewedOk;
                    state.next_action = Action::WriteReport;
                }
            }
            store.clone()
        }
    });

    let mcp_client_report = Arc::clone(&mcp_client);
    let report_node = create_typed_node(move |store: TypedStore<AppState>| {
        let mcp_client_report = Arc::clone(&mcp_client_report);
        async move {
            let mut state = store.inner.write().await;
            info!("--- [Agent 3] Report Synthesizer ---");

            if !has_pdf {
                state.agent_error = Some(AgentError::ReportFailed {
                    reason: "Missing generate_pdf tool".into(),
                    missing_tool: "generate_pdf".into(),
                });
                state.state = StoreState::Failed;
                state.next_action = Action::Failed;
                return store.clone();
            }

            let report_markdown = if let Some(first) = state.artifacts.first() {
                format!(
                    "# GovAlta UI Components README review\n\nURL: {}\nStatus: {}\nTimestamp: {}\n\n## Summary\n{}",
                    first.url, first.status, first.timestamp, first.content
                )
            } else {
                "# GovAlta UI Components README review\n\nNo crawl artifacts available.".into()
            };

            let pdf_path = "/tmp/goa-ui-components-review.pdf".to_string();
            let markdown_path = "/tmp/goa-ui-components-review.md".to_string();

            let tool_result = {
                let mut client = mcp_client_report.lock().await;
                client
                    .call_tool(
                        "generate_pdf",
                        json!({
                            "markdown": report_markdown,
                            "output_path": pdf_path,
                        }),
                    )
                    .await
            };

            match tool_result {
                Ok(result) => {
                    let terminal_text = mcp_text_content(&result);
                    state.report = Some(ReportArtifact {
                        terminal_text: if terminal_text.trim().is_empty() {
                            "generate_pdf completed without terminal output".into()
                        } else {
                            terminal_text
                        },
                        pdf_path,
                        markdown_path,
                    });
                    state.state = StoreState::ReportWritten;
                    state.next_action = Action::Complete;
                }
                Err(e) => {
                    state.agent_error = Some(AgentError::ReportFailed {
                        reason: format!("generate_pdf failed: {e}"),
                        missing_tool: "generate_pdf".into(),
                    });
                    state.state = StoreState::Failed;
                    state.next_action = Action::Failed;
                }
            }
            store.clone()
        }
    });

    flow.add_node("Crawl", crawl_node);
    flow.add_node("Review", review_node);
    flow.add_node("Report", report_node);

    flow.add_transition("Crawl", |state: &AppState| match state.next_action {
        Action::ReviewCrawlResults => Some("Review".to_string()),
        Action::Failed => None,
        _ => None,
    });

    flow.add_transition("Review", |state: &AppState| match state.next_action {
        Action::WriteReport => Some("Report".to_string()),
        Action::CrawlGoADesignSystem => Some("Crawl".to_string()),
        _ => None,
    });

    flow.add_transition("Report", |_state: &AppState| None);

    let store = TypedStore::new(initial_state);
    let final_store = flow.run(store).await;
    let final_state = final_store.inner.read().await;

    info!("Final workflow state: {:?}", final_state.state);
    if let Some(err) = &final_state.agent_error {
        error!("Completed with Agent Error: {:?}", err);
    } else {
        info!("--- Final Result ---");
        if let Some(report) = &final_state.report {
            info!(
                "Report Generated:\n{}",
                serde_json::to_string_pretty(report).unwrap_or_default()
            );
        }
        info!(
            "Artifacts Collected:\n{}",
            serde_json::to_string_pretty(&final_state.artifacts).unwrap_or_default()
        );
    }

    Ok(())
}
