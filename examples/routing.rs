use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    let mut flow = Flow::new();

    // 1. Triage Node: Routes the user request based on intent
    let triage_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let mut guard = store.write().await;
            
            let message = guard.get("message").and_then(|v| v.as_str()).unwrap_or("");
            println!("Triage: Analyzing message: '{}'", message);
            
            // In a real scenario, this would be an LLM or an NLP classifier determining intent.
            let intent = if message.contains("broken") || message.contains("error") {
                "tech_support"
            } else if message.contains("invoice") || message.contains("pay") {
                "billing"
            } else {
                "general"
            };
            
            println!("Triage: Determined intent as '{}'. Routing request...", intent);
            guard.insert("action".to_string(), Value::String(intent.to_string()));
            
            drop(guard);
            store
        })
    });

    // 2. Tech Support Node
    let tech_support_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            println!("Tech Support Agent: Received request. I specialize in fixing technical issues.");
            
            let mut guard = store.write().await;
            guard.insert("response".to_string(), Value::String("Please reboot your computer.".to_string()));
            guard.insert("action".to_string(), Value::String("done".to_string()));
            drop(guard);
            
            store
        })
    });

    // 3. Billing Node
    let billing_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            println!("Billing Agent: Received request. I specialize in accounts and invoices.");
            
            let mut guard = store.write().await;
            guard.insert("response".to_string(), Value::String("I will send you a link to your invoice portal.".to_string()));
            guard.insert("action".to_string(), Value::String("done".to_string()));
            drop(guard);
            
            store
        })
    });

    // 4. General Node
    let general_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            println!("General Agent: Received request. How can I help you today?");
            
            let mut guard = store.write().await;
            guard.insert("response".to_string(), Value::String("Let me assist you with that.".to_string()));
            guard.insert("action".to_string(), Value::String("done".to_string()));
            drop(guard);
            
            store
        })
    });

    // Add nodes
    flow.add_node("triage", triage_node);
    flow.add_node("tech_support", tech_support_node);
    flow.add_node("billing", billing_node);
    flow.add_node("general", general_node);

    // Define edges based on the intent returned by the triage node
    flow.add_edge("triage", "tech_support", "tech_support");
    flow.add_edge("triage", "billing", "billing");
    flow.add_edge("triage", "general", "general");
    
    // Test the Routing flow
    let test_messages = vec![
        "I need to pay my invoice.",
        "My printer is broken.",
        "Hello, how are you?",
    ];

    for (i, msg) in test_messages.iter().enumerate() {
        println!("\n--- Request {} ---", i + 1);
        let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
        
        {
            let mut guard = store.write().await;
            guard.insert("message".to_string(), Value::String(msg.to_string()));
        }
        
        let mut final_store = store.clone();
        
        // Execute the flow starting at triage node
        final_store = flow.run(final_store).await;
        
        let guard = final_store.write().await;
        let response = guard.get("response").and_then(|v| v.as_str()).unwrap_or("");
        println!("Final Output: {}", response);
    }
}