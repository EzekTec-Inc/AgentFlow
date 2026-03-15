use agentflow::core::error::AgentFlowError;
use agentflow::core::{create_typed_node, TypedFlow, TypedStore};
use agentflow::mcp::{McpClient, McpClientOptions};
use rig::client::CompletionClient;
use rig::client::ProviderClient;
use rig::completion::Prompt;
use serde::{Deserialize, Serialize};
use std::env;

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

    info!("Starting AgentFlow Dynamic Orchestrator (GoA Web Research) using TypedFlow");

    // Start MCP Server
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

    // Check available tools
    let mcp_tools = client.list_tools().await?;
    info!("Discovered {} MCP tools", mcp_tools.len());

    let has_crawl = mcp_tools.iter().any(|t| t.name == "crawl_goa_url");
    let has_pdf = mcp_tools.iter().any(|t| t.name == "generate_pdf");

    // Fallback to simple deterministic response if API keys aren't set
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

    // Node 1: Crawl
    let openai_client_crawl = openai_client.clone();
    let crawl_node = create_typed_node(move |store: TypedStore<AppState>| {
        let openai_client_crawl = openai_client_crawl.clone();
        async move {
            let mut state = store.inner.write().await;
            info!("--- [Agent 1] Web Researcher & Crawler ---");

            if state.retry_count > 3 {
                error!("Max retries exceeded. Failing.");
                state.state = StoreState::Failed;
                state.next_action = Action::Failed;
                return store.clone();
            }

            if !has_crawl {
                state.agent_error = Some(AgentError::CrawlFailed {
                    url: "https://raw.githubusercontent.com/GovAlta/ui-components/refs/heads/dev/README.md".into(),
                    reason: "Missing crawl tool".into(),
                    retry_hint: "Add crawl_goa_url tool to MCP".into(),
                });
                state.next_action = Action::Failed;
                return store.clone();
            }

            let agent_1 = openai_client_crawl
                .agent("gpt-4.1-mini")
                .preamble("You are Agent 1: Government of Alberta (GoA), Canada, UI Components Repository Researcher. The user needs you to output exactly a JSON array containing one CrawlArtifact representing the https://raw.githubusercontent.com/GovAlta/ui-components/refs/heads/dev/README.md repository. The JSON should be an array of objects with fields: url, title, content, timestamp, status (as an integer HTTP status code, e.g. 200). No markdown blocks, just raw JSON array.")
                .build();

            match agent_1
                .prompt("Extract information about the GovAlta UI components repository.")
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
                                artifacts.push(CrawlArtifact {
                                    url: "https://raw.githubusercontent.com/GovAlta/ui-components/refs/heads/dev/README.md".into(),
                                    title: "GovAlta UI Components README".into(),
                                    content:
                                        "Use Open Sans for body text. Headings should be clear."
                                            .into(),
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                    status: 200,
                                });
                            }
                            state.artifacts = artifacts;
                            state.state = StoreState::Crawled;
                            state.next_action = Action::ReviewCrawlResults;
                        }
                        Err(e) => {
                            info!("JSON Parse Error: {}. Using deterministic mock payload.", e);
                            state.artifacts = vec![CrawlArtifact {
                                url: "https://raw.githubusercontent.com/GovAlta/ui-components/refs/heads/dev/README.md".into(),
                                title: "GovAlta UI Components README".into(),
                                content: "Use Open Sans for body text. Headings should be clear."
                                    .into(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                status: 200,
                            }];
                            state.state = StoreState::Crawled;
                            state.next_action = Action::ReviewCrawlResults;
                        }
                    }
                }
                Err(e) => {
                    info!("LLM Prompt failed (likely no API key): {}. Using deterministic mock payload.", e);
                    state.artifacts = vec![CrawlArtifact {
                        url: "https://raw.githubusercontent.com/GovAlta/ui-components/refs/heads/dev/README.md".into(),
                        title: "GovAlta UI Components README".into(),
                        content: "Use Open Sans for body text. Headings should be clear.".into(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        status: 200,
                    }];
                    state.state = StoreState::Crawled;
                    state.next_action = Action::ReviewCrawlResults;
                }
            }
            store.clone()
        }
    });

    // Node 2: Review
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
                            info!(
                                "Evaluator JSON Parse Error: {}. Proceeding to WriteReport.",
                                e
                            );
                            state.state = StoreState::ReviewedOk;
                            state.next_action = Action::WriteReport;
                        }
                    }
                }
                Err(e) => {
                    info!("Evaluator LLM Prompt failed: {}. Proceeding.", e);
                    state.state = StoreState::ReviewedOk;
                    state.next_action = Action::WriteReport;
                }
            }
            store.clone()
        }
    });

    // Node 3: Report
    let openai_client_report = openai_client.clone();
    let report_node = create_typed_node(move |store: TypedStore<AppState>| {
        let openai_client_report = openai_client_report.clone();
        async move {
            let mut state = store.inner.write().await;
            info!("--- [Agent 3] Report Synthesizer ---");

            if !has_pdf {
                state.agent_error = Some(AgentError::ReportFailed {
                    reason: "Missing generate_pdf tool".into(),
                    missing_tool: "generate_pdf".into(),
                });
                state.next_action = Action::Failed;
                return store.clone();
            }

            let agent_3 = openai_client_report
                .agent("gpt-5.4")
                .preamble("You are Agent 3: Report Synthesizer. Output exactly a JSON object: {\"terminal_text\":\"Done!\", \"pdf_path\":\"report.pdf\", \"markdown_path\":\"report.md\"}")
                .build();

            let payload = serde_json::to_string(&state.artifacts).unwrap_or_default();
            match agent_3
                .prompt(&format!("Synthesize report for: {}", payload))
                .await
            {
                Ok(res) => {
                    let clean_res = res
                        .trim()
                        .trim_start_matches("```json")
                        .trim_end_matches("```")
                        .trim();
                    match serde_json::from_str::<ReportArtifact>(clean_res) {
                        Ok(report) => {
                            state.report = Some(report);
                            state.state = StoreState::ReportWritten;
                            state.next_action = Action::Complete;
                        }
                        Err(e) => {
                            info!("Report JSON Parse Error: {}. Using fallback.", e);
                            state.report = Some(ReportArtifact {
                                terminal_text: "Synthesis complete (fallback)".into(),
                                pdf_path: "report.pdf".into(),
                                markdown_path: "report.md".into(),
                            });
                            state.state = StoreState::ReportWritten;
                            state.next_action = Action::Complete;
                        }
                    }
                }
                Err(e) => {
                    info!("Report LLM failed: {}. Using fallback.", e);
                    state.report = Some(ReportArtifact {
                        terminal_text: "Synthesis complete (fallback)".into(),
                        pdf_path: "report.pdf".into(),
                        markdown_path: "report.md".into(),
                    });
                    state.state = StoreState::ReportWritten;
                    state.next_action = Action::Complete;
                }
            }
            store.clone()
        }
    });

    // Build the Flow structure
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
