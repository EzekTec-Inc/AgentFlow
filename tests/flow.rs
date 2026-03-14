use agentflow::core::error::AgentFlowError;
use agentflow::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_flow_linear_routing() {
    let mut flow = Flow::new();

    let a = create_node(|store| async move {
        store
            .write()
            .await
            .insert("visited_a".into(), serde_json::json!(true));
        store
            .write()
            .await
            .insert("action".into(), serde_json::json!("next"));
        store
    });

    let b = create_node(|store| async move {
        store
            .write()
            .await
            .insert("visited_b".into(), serde_json::json!(true));
        // no action = stop
        store
    });

    flow.add_node("A", a);
    flow.add_node("B", b);
    flow.add_edge("A", "next", "B");

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    let result = flow.run(store).await;

    let state = result.read().await;
    assert!(state.contains_key("visited_a"));
    assert!(state.contains_key("visited_b"));
    assert!(
        !state.contains_key("action"),
        "Action key should be cleaned up"
    );
}

#[tokio::test]
async fn test_flow_max_steps_cycle() {
    let mut flow = Flow::new().with_max_steps(3);

    let node = create_node(|store| async move {
        let count = {
            let guard = store.read().await;
            guard.get("count").and_then(|v| v.as_i64()).unwrap_or(0)
        };
        store
            .write()
            .await
            .insert("count".into(), serde_json::json!(count + 1));
        store
            .write()
            .await
            .insert("action".into(), serde_json::json!("loop"));
        store
    });

    flow.add_node("A", node);
    flow.add_edge("A", "loop", "A");

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    let result = flow.run(store).await;

    let state = result.read().await;
    assert_eq!(state.get("count").and_then(|v| v.as_i64()), Some(3));
    assert_eq!(
        state.get("error").and_then(|v| v.as_str()),
        Some("Flow execution exceeded max_steps limit")
    );
}

#[tokio::test]
async fn test_flow_run_safe_cycle() {
    let mut flow = Flow::new().with_max_steps(2);

    let node = create_node(|store| async move {
        store
            .write()
            .await
            .insert("action".into(), serde_json::json!("loop"));
        store
    });

    flow.add_node("A", node);
    flow.add_edge("A", "loop", "A");

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    let result = flow.run_safe(store).await;

    match result {
        Err(AgentFlowError::ExecutionLimitExceeded(_)) => {} // Expected
        _ => panic!("Expected ExecutionLimitExceeded error"),
    }
}
