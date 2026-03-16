use agentflow::core::node::Node;
use agentflow::utils::tool::create_tool_node_with_timeout;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[tokio::test]
async fn tool_node_timeout_reports_error_and_returns_promptly() {
    let node = create_tool_node_with_timeout(
        "sleep_test",
        "python3",
        vec!["-c".into(), "import time; time.sleep(5)".into()],
        Duration::from_millis(100),
    );

    let store = Arc::new(RwLock::new(HashMap::new()));
    let start = tokio::time::Instant::now();
    let result = node.call(store).await;
    let elapsed = start.elapsed();

    assert!(elapsed < Duration::from_secs(2));

    let guard = result.read().await;
    assert_eq!(
        guard.get("sleep_test_status").and_then(|v| v.as_i64()),
        Some(-1)
    );
    assert!(guard
        .get("sleep_test_error")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .contains("timed out"));
}
