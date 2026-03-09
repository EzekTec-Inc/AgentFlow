use agentflow::core::node::{create_node, SharedStore};
use agentflow::patterns::rpi::RpiWorkflow;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    // 1. Research Node: Gathers context and information needed for the task
    let research_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let mut guard = store.write().await;
            
            let goal = guard.get("goal").and_then(|v| v.as_str()).unwrap_or("Unknown task").to_string();
            println!("Research: Investigating goal: '{}'", goal);
            
            // Simulate researching context (e.g., searching documentation)
            let context = format!("Found documentation relevant to: {}", goal);
            guard.insert("context".to_string(), Value::String(context));
            
            // Move to the next default step
            guard.insert("action".to_string(), Value::String("default".to_string()));
            
            drop(guard);
            store
        })
    });

    // 2. Plan Node: Decides the step-by-step execution strategy
    let plan_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let mut guard = store.write().await;
            
            let context = guard.get("context").and_then(|v| v.as_str()).unwrap_or("").to_string();
            println!("Plan: Formulating a plan based on context: '{}'", context);
            
            // Simulate planning based on the research
            let plan = "1. Setup environment. 2. Write code. 3. Run tests.";
            guard.insert("plan".to_string(), Value::String(plan.to_string()));
            
            guard.insert("action".to_string(), Value::String("default".to_string()));
            
            drop(guard);
            store
        })
    });

    // 3. Implement Node: Executes the plan (e.g., writing code, calling tools)
    let implement_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let mut guard = store.write().await;
            
            let plan = guard.get("plan").and_then(|v| v.as_str()).unwrap_or("").to_string();
            println!("Implement: Executing plan: '{}'", plan);
            
            // Track how many times we've tried to implement
            let attempt = guard.get("attempt").and_then(|v| v.as_i64()).unwrap_or(0) + 1;
            guard.insert("attempt".to_string(), Value::Number(attempt.into()));
            
            let output = if attempt == 1 {
                "Code written, but it has a compilation error."
            } else {
                "Code written perfectly, all tests pass."
            };
            
            guard.insert("implementation_output".to_string(), Value::String(output.to_string()));
            guard.insert("action".to_string(), Value::String("default".to_string()));
            
            drop(guard);
            store
        })
    });

    // 4. Verify Node: Checks the implementation output
    let verify_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let mut guard = store.write().await;
            
            let output = guard.get("implementation_output").and_then(|v| v.as_str()).unwrap_or("");
            println!("Verify: Checking implementation output: '{}'", output);
            
            if output.contains("error") {
                println!("Verify: Found errors. Sending back to implement for a fix.");
                guard.insert("action".to_string(), Value::String("reimplement".to_string()));
            } else {
                println!("Verify: Everything looks good! Task complete.");
                // Emitting anything other than replan/reimplement ends the default RPI flow
                guard.insert("action".to_string(), Value::String("done".to_string()));
            }
            
            drop(guard);
            store
        })
    });

    // Assemble the RPI Workflow
    let rpi_workflow = RpiWorkflow::new()
        .with_research(research_node)
        .with_plan(plan_node)
        .with_implement(implement_node)
        .with_verify(verify_node);

    // Initialize state
    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    {
        let mut guard = store.write().await;
        guard.insert("goal".to_string(), Value::String("Create a Rust HTTP server".to_string()));
    }
    
    println!("Starting RPI Workflow Pattern...");
    let final_store = rpi_workflow.run(store).await;
    
    let guard = final_store.write().await;
    println!("\nRPI Workflow completed.");
    println!("Final Context: {:?}", guard.get("context").and_then(|v| v.as_str()));
    println!("Final Plan: {:?}", guard.get("plan").and_then(|v| v.as_str()));
    println!("Final Implementation Output: {:?}", guard.get("implementation_output").and_then(|v| v.as_str()));
}