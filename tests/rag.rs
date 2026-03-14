use agentflow::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_rag_pipeline() {
    let retriever = create_node(|store| async move {
        let query = store
            .read()
            .await
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        store.write().await.insert(
            "context".into(),
            serde_json::json!(format!("Found doc for {}", query)),
        );
        store
    });

    let generator = create_node(|store| async move {
        let ctx = store
            .read()
            .await
            .get("context")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        store.write().await.insert(
            "response".into(),
            serde_json::json!(format!("Answer: {}", ctx)),
        );
        store
    });

    let rag = Rag::new(retriever, generator);

    let mut init = HashMap::new();
    init.insert("query".into(), serde_json::json!("Rust"));
    let store: SharedStore = Arc::new(RwLock::new(init));

    let result = rag.call(store).await;

    let state = result.read().await;
    assert_eq!(
        state.get("response").and_then(|v| v.as_str()),
        Some("Answer: Found doc for Rust")
    );
}
