use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    let mut flow = Flow::new();

    // 1. Generator Node: Creates an initial draft or revises it based on feedback.
    let generator_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let mut guard = store.write().await;
            
            let attempt = guard
                .get("attempt")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) + 1;
                
            guard.insert("attempt".to_string(), Value::Number(attempt.into()));
            
            let feedback = guard.get("feedback").and_then(|v| v.as_str());
            
            let draft = if let Some(f) = feedback {
                println!("Generator: Revising draft based on feedback: '{}'", f);
                "This is the revised, perfect draft.".to_string()
            } else {
                println!("Generator: Creating initial draft...");
                "This is the first draft. It has some flaws.".to_string()
            };
            
            guard.insert("draft".to_string(), Value::String(draft));
            guard.insert("action".to_string(), Value::String("critique".to_string()));
            
            drop(guard);
            store
        })
    });

    // 2. Critic Node: Evaluates the draft.
    let critic_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let mut guard = store.write().await;
            
            let attempt = guard.get("attempt").and_then(|v| v.as_i64()).unwrap_or(1);
            
            println!("Critic: Reviewing attempt #{}...", attempt);
            
            let (next_action, feedback) = if attempt < 2 {
                println!("Critic: Draft is not good enough. Sending back for revision.");
                ("revise", Some("Please make the tone more professional."))
            } else {
                println!("Critic: Draft looks great! Approving.");
                ("approve", None)
            };
            
            guard.insert("action".to_string(), Value::String(next_action.to_string()));
            if let Some(f) = feedback {
                guard.insert("feedback".to_string(), Value::String(f.to_string()));
            }
            
            drop(guard);
            store
        })
    });

    // Add nodes
    flow.add_node("generator", generator_node);
    flow.add_node("critic", critic_node);

    // Define edges
    flow.add_edge("generator", "critique", "critic");
    // If the critic says revise, loop back to the generator
    flow.add_edge("critic", "revise", "generator"); 
    // If approved, the flow naturally terminates as there is no edge for "approve"

    let store = Arc::new(RwLock::new(HashMap::new()));
    
    println!("Starting Reflection Pattern...");
    let final_store = flow.run(store).await;
    
    let guard = final_store.write().await;
    let final_draft = guard.get("draft").and_then(|v| v.as_str()).unwrap_or("");
    println!("Reflection completed. Final output: {}", final_draft);
}