use agentflow::core::error::AgentFlowError;
use agentflow::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_agent_retry_on_error_key() {
    let node = create_node(|store| async move {
        {
            let attempts = {
                let guard = store.read().await;
                guard.get("attempts").and_then(|v| v.as_i64()).unwrap_or(0)
            } + 1;

            let mut guard = store.write().await;
            guard.insert("attempts".into(), serde_json::json!(attempts));

            if attempts < 3 {
                guard.insert("error".into(), serde_json::json!("Need more attempts"));
            } else {
                guard.remove("error");
                guard.insert("success".into(), serde_json::json!(true));
            }
        } //NOTE: Stephen Ezekwem - March 18, 11:53 AM, 2026: Putting the sharedstore (Arc) into
          //curly braces is a drop guard as well.
          // drop(guard);
        store
    });

    // 4 retries max, 0 delay
    let agent = Agent::with_retry(node, 4, 0);

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    let result = agent.decide_shared(store).await;

    let state = result.read().await;
    assert_eq!(state.get("attempts").and_then(|v| v.as_i64()), Some(3));
    assert!(state.contains_key("success"));
    assert!(!state.contains_key("error"));
}

#[tokio::test]
async fn test_agent_decide_result_transient_retry() {
    let node = create_result_node(|store| async move {
        let attempts = {
            let guard = store.read().await;
            guard.get("attempts").and_then(|v| v.as_i64()).unwrap_or(0)
        } + 1;

        store
            .write()
            .await
            .insert("attempts".into(), serde_json::json!(attempts));

        if attempts < 2 {
            Err(AgentFlowError::Timeout("Network blip".into()))
        } else {
            Ok(store)
        }
    });

    // The Agent itself requires an inner SimpleNode (Node<SharedStore, SharedStore>) for standard operation,
    // but decide_result lets us pass an explicit NodeResult.
    // To construct the Agent, we can just pass a dummy SimpleNode.
    let dummy_node = create_node(|s| async move { s });
    let agent = Agent::with_retry(dummy_node, 3, 0);

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    let result = agent.decide_result(store, &node).await.unwrap();

    let state = result.read().await;
    assert_eq!(state.get("attempts").and_then(|v| v.as_i64()), Some(2));
}

#[tokio::test]
async fn test_agent_decide_result_fatal_error() {
    let node = create_result_node(|_store| async move {
        Err(AgentFlowError::NodeFailure("Hard crash".into()))
    });

    let dummy_node = create_node(|s| async move { s });
    let agent = Agent::with_retry(dummy_node, 3, 0);

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    let result = agent.decide_result(store, &node).await;

    match result {
        Err(AgentFlowError::NodeFailure(msg)) => assert_eq!(msg, "Hard crash"),
        _ => panic!("Expected NodeFailure"),
    }
}
