use agentflow::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_mapreduce_flow() {
    let mapper = create_node(|store| async move {
        let count = store.read().await.get("val").and_then(|v| v.as_i64()).unwrap_or(0);
        store.write().await.insert("mapped".into(), serde_json::json!(count * 2));
        store
    });

    let reducer = create_batch_node(|stores: Vec<SharedStore>| async move {
        let mut sum = 0;
        for store in stores {
            sum += store.read().await.get("mapped").and_then(|v| v.as_i64()).unwrap_or(0);
        }
        let out: SharedStore = Arc::new(RwLock::new(HashMap::new()));
        out.write().await.insert("total".into(), serde_json::json!(sum));
        out
    });

    let mr = MapReduce::new(Batch::new(mapper), reducer);

    let mut inputs = Vec::new();
    for i in 1..=3 {
        let mut init = HashMap::new();
        init.insert("val".into(), serde_json::json!(i)); // 1, 2, 3
        inputs.push(Arc::new(RwLock::new(init)));
    }

    let result = mr.run(inputs).await;
    
    // sum of (1*2 + 2*2 + 3*2) = 2 + 4 + 6 = 12
    let state = result.read().await;
    assert_eq!(state.get("total").and_then(|v| v.as_i64()), Some(12));
}
