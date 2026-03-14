use agentflow::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_multi_agent_shared_store() {
    let mut multi = MultiAgent::with_strategy(MergeStrategy::SharedStore);

    multi.add_agent(create_node(|store| async move {
        store
            .write()
            .await
            .insert("agent1".into(), serde_json::json!(true));
        store
    }));

    multi.add_agent(create_node(|store| async move {
        store
            .write()
            .await
            .insert("agent2".into(), serde_json::json!(true));
        store
    }));

    let store = Arc::new(RwLock::new(HashMap::new()));
    let result = multi.run(store).await;

    let state = result.read().await;
    assert!(state.contains_key("agent1"));
    assert!(state.contains_key("agent2"));
}

#[tokio::test]
async fn test_multi_agent_namespaced() {
    let mut multi = MultiAgent::with_strategy(MergeStrategy::Namespaced);

    multi.add_agent(create_node(|store| async move {
        store
            .write()
            .await
            .insert("result".into(), serde_json::json!("data1"));
        store
    }));

    multi.add_agent(create_node(|store| async move {
        store
            .write()
            .await
            .insert("result".into(), serde_json::json!("data2"));
        store
    }));

    let store = Arc::new(RwLock::new(HashMap::new()));
    let result = multi.run(store).await;

    let state = result.read().await;
    assert_eq!(
        state.get("agent_0.result").and_then(|v| v.as_str()),
        Some("data1")
    );
    assert_eq!(
        state.get("agent_1.result").and_then(|v| v.as_str()),
        Some("data2")
    );
}
