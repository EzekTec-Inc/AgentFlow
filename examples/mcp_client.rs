use agentflow::core::error::AgentFlowError;
use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use rig::completion::Prompt;
use rig::prelude::*;
use rig::providers::openai;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::ServiceExt;
use serde_json::Value;
use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<(), AgentFlowError> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting AgentFlow MCP Client with rig and gpt-4.1-mini...");

    // Find the mcp_server executable in the same directory as this client executable
    let mut server_exe = env::current_exe().map_err(|e| AgentFlowError::Custom(e.to_string()))?;
    server_exe.set_file_name("mcp-server");
    if cfg!(windows) {
        server_exe.set_extension("exe");
    }

    info!("Spawning MCP Server from {:?}", server_exe);

    // Spawn the server and create rmcp client transport
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

    // 1. Setup a Node that uses Rig and GPT-4.1-mini to execute MCP tools
    let rig_llm_node = create_node({
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
                    .preamble("You are a system administrator. Use the provided date_tool to get the current date and output a nice greeting with the date.");

                // Register all MCP tools with the rig agent
                for tool in mcp_tools {
                    builder = builder.rmcp_tool(tool, client.clone());
                }
                let agent = builder.build();

                info!("Prompting gpt-4.1-mini via Rig...");
                let response: String = agent
                    .prompt("Tell me the current date.")
                    .multi_turn(3)
                    .await
                    .unwrap_or_else(|e| format!("LLM Error: {}", e));
                
                store
                    .write()
                    .await
                    .insert("llm_response".to_string(), Value::String(response));
                store
            })
        }
    });

    // 2. Build the Flow graph
    let mut flow = Flow::new().with_max_steps(5);
    flow.add_node("rig_agent", rig_llm_node);
    
    // 3. Execute the Flow
    let store = Arc::new(RwLock::new(std::collections::HashMap::new()));
    let final_store = flow.run(store).await;

    let guard = final_store.read().await;
    let result = guard.get("llm_response").and_then(|v| v.as_str()).unwrap_or("No response");
    
    println!("\n=== MCP Rig Execution Complete ===");
    println!("Agent Output:\n{}", result);

    // Clean up MCP Server
    client
        .cancel()
        .await
        .map_err(|e| AgentFlowError::Custom(format!("{:?}", e)))?;

    Ok(())
}