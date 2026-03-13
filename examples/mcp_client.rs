use agentflow::core::error::AgentFlowError;
use agentflow::patterns::Agent as AgentFlowAgent;
use agentflow::prelude::*;
use rig::completion::Prompt;
use rig::prelude::*;
use rig::providers::openai;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::ServiceExt;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use tracing::{info, Level};

async fn get_string_from_store(store: &SharedStore, key: &str) -> String {
    let guard = store.write().await;
    guard
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

#[tokio::main]
async fn main() -> Result<(), AgentFlowError> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting AgentFlow MCP Client Orchestrator...");

    // Find the mcp_server executable in the same directory as this client executable
    let mut server_exe = env::current_exe().map_err(|e| AgentFlowError::Custom(e.to_string()))?;
    server_exe.set_file_name("mcp-server");
    if cfg!(windows) {
        server_exe.set_extension("exe");
    }

    info!("Spawning MCP Server from {:?}", server_exe);

    let transport = TokioChildProcess::new(tokio::process::Command::new(server_exe))
        .map_err(|e| AgentFlowError::Custom(e.to_string()))?;

    // Connect using rmcp
    let client = ().serve(transport).await.map_err(|e| AgentFlowError::Custom(format!("{:?}", e)))?;

    // Fetch tools from the MCP server
    let mcp_tools = client
        .list_all_tools()
        .await
        .map_err(|e| AgentFlowError::Custom(format!("{:?}", e)))?;
    info!("Discovered {} tools from MCP Server", mcp_tools.len());

    let openai_client = openai::Client::from_env();

    // Setup 3 Agents using the Orchestrator Pattern

    // Agent 1: System Time Fetcher (uses date_tool)
    let time_fetcher = create_node({
        let openai_client = openai_client.clone();
        let mcp_tools = mcp_tools.clone();
        let client = client.clone();
        move |store: SharedStore| {
            let openai_client = openai_client.clone();
            let mcp_tools = mcp_tools.clone();
            let client = client.clone();
            Box::pin(async move {
                let mut builder = openai_client
                    .agent("gpt-4.1-mini")
                    .preamble("You are a system metrics researcher. Use the date_tool to get the current system time, then output ONLY the exact time string you receive.");

                for tool in mcp_tools {
                    builder = builder.rmcp_tool(tool, client.clone());
                }
                let agent = builder.build();

                let response = agent
                    .prompt("Get the current time using the tool.")
                    .await
                    .unwrap_or_else(|e| format!("Error: {}", e));
                store
                    .write()
                    .await
                    .insert("system_time".to_string(), Value::String(response));
                store
            })
        }
    });

    // Agent 2: System Logger (uses echo_tool)
    let system_logger = create_node({
        let openai_client = openai_client.clone();
        let mcp_tools = mcp_tools.clone();
        let client = client.clone();
        move |store: SharedStore| {
            let openai_client = openai_client.clone();
            let mcp_tools = mcp_tools.clone();
            let client = client.clone();
            Box::pin(async move {
                let time = get_string_from_store(&store, "system_time").await;
                let mut builder = openai_client
                    .agent("gpt-4.1")
                    .preamble("You are a system logger. Use the echo_tool to broadcast the system time. Output a summary of what you echoed.");

                for tool in mcp_tools {
                    builder = builder.rmcp_tool(tool, client.clone());
                }
                let agent = builder.build();

                let prompt_text = format!("Echo the following time: {}", time);
                let response = agent
                    .prompt(&prompt_text)
                    .await
                    .unwrap_or_else(|e| format!("Error: {}", e));
                store
                    .write()
                    .await
                    .insert("log_summary".to_string(), Value::String(response));
                store
            })
        }
    });

    // Agent 3: System Reviewer (no tools needed, just reviews the process)
    let system_reviewer = create_node({
        let openai_client = openai_client.clone();
        move |store: SharedStore| {
            let openai_client = openai_client.clone();
            Box::pin(async move {
                let time = get_string_from_store(&store, "system_time").await;
                let summary = get_string_from_store(&store, "log_summary").await;

                let agent = openai_client
                    .agent("gpt-4.1-mini")
                    .preamble("You are an IT auditor. Review the logged time and the summary. Keep your review brief.")
                    .build();

                let prompt_text = format!(
                    "Time logged: {}\nSummary: {}\nProvide an audit report.",
                    time, summary
                );
                let response = agent
                    .prompt(&prompt_text)
                    .await
                    .unwrap_or_else(|e| format!("Error: {}", e));
                store
                    .write()
                    .await
                    .insert("audit_report".to_string(), Value::String(response));
                store
            })
        }
    });

    // Orchestrator Node
    let orchestrator = create_node(move |store: SharedStore| {
        let time_fetcher = time_fetcher.clone();
        let system_logger = system_logger.clone();
        let system_reviewer = system_reviewer.clone();

        Box::pin(async move {
            info!("Running Agent 1: Time Fetcher...");
            let store = time_fetcher.call(store).await;

            info!("Running Agent 2: System Logger...");
            let store = system_logger.call(store).await;

            info!("Running Agent 3: System Reviewer...");
            let store = system_reviewer.call(store).await;

            let time = get_string_from_store(&store, "system_time").await;
            let summary = get_string_from_store(&store, "log_summary").await;
            let report = get_string_from_store(&store, "audit_report").await;

            println!("\n=== MCP Orchestrator Workflow Complete ===");
            println!("Time Fetcher Output:\n{}\n", time);
            println!("System Logger Summary:\n{}\n", summary);
            println!("Audit Report:\n{}\n", report);

            store
        })
    });

    // Run the workflow
    let agent = AgentFlowAgent::new(orchestrator);
    let initial_store = HashMap::new();
    let _ = agent.decide(initial_store).await;

    // Clean up MCP Server
    client
        .cancel()
        .await
        .map_err(|e| AgentFlowError::Custom(format!("{:?}", e)))?;

    Ok(())
}
