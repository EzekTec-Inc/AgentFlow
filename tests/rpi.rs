use agentflow::patterns::rpi::{create_rpi_workflow, RpiWorkflow};
use agentflow::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_rpi_workflow_happy_path() {
    let research = create_node(|s| async move {
        s.write().await.insert("r".into(), serde_json::json!(true));
        s
    });
    let plan = create_node(|s| async move {
        s.write().await.insert("p".into(), serde_json::json!(true));
        s
    });
    let implement = create_node(|s| async move {
        s.write().await.insert("i".into(), serde_json::json!(true));
        s
    });
    let verify = create_node(|s| async move {
        s.write().await.insert("v".into(), serde_json::json!(true));
        // Default action: ends flow
        s
    });

    let rpi = create_rpi_workflow(research, plan, implement, verify);
    
    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    let result = rpi.run(store).await;
    
    let state = result.read().await;
    assert!(state.contains_key("r"));
    assert!(state.contains_key("p"));
    assert!(state.contains_key("i"));
    assert!(state.contains_key("v"));
}

#[tokio::test]
async fn test_rpi_workflow_replan_loop() {
    // We use a counter to only replan once
    let research = create_node(|s| async move { s });
    let plan = create_node(|s| async move {
        let count = s.read().await.get("plan_count").and_then(|v| v.as_i64()).unwrap_or(0);
        s.write().await.insert("plan_count".into(), serde_json::json!(count + 1));
        s
    });
    let implement = create_node(|s| async move { s });
    let verify = create_node(|s| async move {
        let count = s.read().await.get("plan_count").and_then(|v| v.as_i64()).unwrap_or(0);
        if count == 1 {
            // First time: replan
            s.write().await.insert("action".into(), serde_json::json!("replan"));
        } else {
            // Second time: finish
            s.write().await.remove("action");
        }
        s
    });

    let rpi = RpiWorkflow::new()
        .with_research(research)
        .with_plan(plan)
        .with_implement(implement)
        .with_verify(verify);
        
    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    let result = rpi.run(store).await;
    
    let state = result.read().await;
    assert_eq!(state.get("plan_count").and_then(|v| v.as_i64()), Some(2));
}
