use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    let mut flow = Flow::new();

    // 1. Reasoner Node: Analyzes the current context and decides if a tool is needed or if it can answer.
    let reasoner_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let mut guard = store.write().await;
            
            let question = guard.get("question").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let tool_output = guard.get("tool_output").and_then(|v| v.as_str()).map(|s| s.to_string());
            
            println!("Reasoner: Thinking about question: '{}'", question);
            
            if let Some(output) = tool_output {
                println!("Reasoner: Analyzing tool output: '{}'", output);
                // We have the answer now
                guard.insert("final_answer".to_string(), Value::String(format!("Based on {}, the answer is clear.", output)));
                guard.insert("action".to_string(), Value::String("done".to_string()));
            } else {
                println!("Reasoner: I don't know the answer. I need to use the search tool.");
                // Set the tool to call
                guard.insert("tool_name".to_string(), Value::String("search".to_string()));
                guard.insert("tool_query".to_string(), Value::String(question));
                guard.insert("action".to_string(), Value::String("use_tool".to_string()));
            }
            
            drop(guard);
            store
        })
    });

    // 2. Tool Node: Executes the requested tool.
    let tool_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let mut guard = store.write().await;
            
            if let Some(tool) = guard.get("tool_name").and_then(|v| v.as_str()) {
                if tool == "search" {
                    let query = guard.get("tool_query").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    println!("ToolExecutor: Running 'search' tool for query: '{}'", query);
                    
                    // Simulate a tool execution (e.g., calling an API)
                    let simulated_result = format!("Search results for {}", query);
                    guard.insert("tool_output".to_string(), Value::String(simulated_result));
                }
            }
            
            // Go back to reasoning with the new information
            guard.insert("action".to_string(), Value::String("reason".to_string()));
            
            drop(guard);
            store
        })
    });

    // Add nodes
    flow.add_node("reasoner", reasoner_node);
    flow.add_node("tool_executor", tool_node);

    // Define edges
    flow.add_edge("reasoner", "use_tool", "tool_executor");
    flow.add_edge("tool_executor", "reason", "reasoner");
    // If reasoner says "done", it stops as there's no outgoing edge.

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    
    // Set initial context
    {
        let mut guard = store.write().await;
        guard.insert("question".to_string(), Value::String("What is the capital of France?".to_string()));
    }
    
    println!("Starting ReAct Pattern...");
    let final_store = flow.run(store).await;
    
    let guard = final_store.write().await;
    if let Some(answer) = guard.get("final_answer").and_then(|v| v.as_str()) {
        println!("ReAct completed. Final answer: {}", answer);
    }
}