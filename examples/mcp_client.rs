use agentflow::core::error::AgentFlowError;
use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use agentflow::patterns::agent::Agent;
use agentflow::utils::tool::create_tool_node;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), AgentFlowError> {
    let mut flow = Flow::new().with_max_steps(10);

    // 1. Showcase AgentFlow's native Tool Node (The "Hands")
    // Instead of raw RMCP traits, use AgentFlow's tool wrapper to call the MCP server
    let call_mcp_tool = create_tool_node(
        "mcp_caller",
        "cargo", // Calls the native server built in mcp_server.rs
        vec!["run".into(), "--example".into(), "mcp_server".into(), "--quiet".into()]
    );

    // 2. Wrap it in AgentFlow's Agent primitive for automatic error retries
    // This showcases built-in fault tolerance
    let fault_tolerant_mcp_caller = Agent::with_retry(call_mcp_tool, 3, 500);

    // 3. Build a planning node to write the JSON-RPC payload into the store
    let payload_builder = create_node(|store: SharedStore| {
        Box::pin(async move {
            let payload = serde_json::json!({
                "jsonrpc": "2.0", 
                "id": 1, 
                "method": "tools/list"
            });
            
            let mut g = store.write().await;
            // The tool node natively reads from mcp_caller_stdin when configured
            g.insert("mcp_caller_stdin".into(), Value::String(payload.to_string() + "\n"));
            g.insert("action".into(), Value::String("execute".into()));
            // Drop the guard before returning the store
            drop(g);
            store
        })
    });

    // 4. Wrap the Agent in a SimpleNode closure to fit the Flow
    let executor = create_node(move |store: SharedStore| {
        let agent = fault_tolerant_mcp_caller.clone();
        Box::pin(async move {
            let next_store = agent.decide_shared(store).await;
            next_store
        })
    });

    // 5. Graph Orchestration
    flow.add_node("planner", payload_builder);
    flow.add_node("executor", executor);
    flow.add_edge("planner", "execute", "executor");

    let store = std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
    
    // 6. Execute Flow
    let final_store = flow.run(store).await;

    // The output natively arrives in `mcp_caller_stdout` via AgentFlow
    println!("MCP Output: {:?}", final_store.read().await.get("mcp_caller_stdout"));
    
    Ok(())
}
