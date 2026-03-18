use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use agentflow::core::error::AgentFlowError;
use agentflow::core::flow::Flow;
use agentflow::core::node::create_node;
use agentflow::patterns::hitl::create_hitl_node;
use serde_json::json;

#[tokio::test]
async fn test_hitl_suspends_when_input_missing() {
    let mut flow = Flow::new();

    flow.add_node("step1", create_node(|store| async move { store }));
    
    // HITL node looking for "human_approval" key
    flow.add_result_node(
        "hitl",
        create_hitl_node("human_approval", "step3", "Awaiting human approval"),
    );
    
    flow.add_node("step3", create_node(|store| async move { store }));

    flow.add_edge("step1", "default", "hitl");

    let store = Arc::new(RwLock::new(HashMap::new()));
    
    // Run safe should return Err(Suspended) after step1 finishes and hits hitl
    // Wait, Flow::run_safe runs the flow starting from start_node.
    flow.set_start("step1");

    let result = flow.run_safe(store).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        AgentFlowError::Suspended(reason) => {
            assert_eq!(reason, "Awaiting human approval");
        }
        _ => panic!("Expected AgentFlowError::Suspended"),
    }
}

#[tokio::test]
async fn test_hitl_proceeds_when_input_present() {
    let mut flow = Flow::new();

    flow.add_node("step1", create_node(|store| async move { println!("step1 ran"); store }));
    
    // HITL node looking for "human_approval" key
    flow.add_result_node(
        "hitl",
        create_hitl_node("human_approval", "step3", "Awaiting human approval"),
    );
    
    flow.add_node("step3", create_node(|store| async move {
        println!("step3 ran");
        let mut guard = store.write().await;
        guard.insert("final_step_reached".to_string(), json!(true));
        drop(guard);
        store
    }));

    flow.add_edge("step1", "default", "hitl");
    flow.add_edge("hitl", "step3", "step3");

    flow.set_start("step1");

    let mut map = HashMap::new();
    map.insert("human_approval".to_string(), json!(true));
    let store = Arc::new(RwLock::new(map));

    let result = flow.run_safe(store.clone()).await;

    assert!(result.is_ok());
    let final_store = result.unwrap();
    let guard = final_store.read().await;
    println!("STORE: {:?}", *guard);
    assert_eq!(guard.get("final_step_reached"), Some(&json!(true)));
}
