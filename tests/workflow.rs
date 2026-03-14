use agentflow::prelude::*;
use std::collections::HashMap;

#[tokio::test]
async fn test_workflow_linear_execution() {
    let mut wf = Workflow::new();

    wf.add_step(
        "step1",
        create_node(|store| async move {
            store
                .write()
                .await
                .insert("s1".into(), serde_json::json!(true));
            store
        }),
    );

    wf.add_step(
        "step2",
        create_node(|store| async move {
            store
                .write()
                .await
                .insert("s2".into(), serde_json::json!(true));
            store
        }),
    );

    wf.connect("step1", "step2");

    let result = wf.execute(HashMap::new()).await;

    assert!(result.contains_key("s1"));
    assert!(result.contains_key("s2"));
}

#[tokio::test]
async fn test_workflow_conditional_branching() {
    let mut wf = Workflow::new();

    wf.add_step(
        "router",
        create_node(|store| async move {
            store
                .write()
                .await
                .insert("action".into(), serde_json::json!("path_b"));
            store
        }),
    );

    wf.add_step(
        "path_a",
        create_node(|store| async move {
            store
                .write()
                .await
                .insert("visited".into(), serde_json::json!("A"));
            store
        }),
    );

    wf.add_step(
        "path_b",
        create_node(|store| async move {
            store
                .write()
                .await
                .insert("visited".into(), serde_json::json!("B"));
            store
        }),
    );

    wf.connect_with_action("router", "path_a", "path_a");
    wf.connect_with_action("router", "path_b", "path_b");

    let result = wf.execute(HashMap::new()).await;

    assert_eq!(result.get("visited").and_then(|v| v.as_str()), Some("B"));
}

#[tokio::test]
async fn test_workflow_params_merge() {
    let mut wf = Workflow::new();

    let mut default_params = HashMap::new();
    default_params.insert("default_key".into(), serde_json::json!("default_val"));
    default_params.insert("override_me".into(), serde_json::json!("old_val"));
    wf.set_params(default_params);

    wf.add_step("read", create_node(|store| async move { store }));

    let mut init = HashMap::new();
    init.insert("override_me".into(), serde_json::json!("new_val"));

    let result = wf.execute(init).await;

    assert_eq!(
        result.get("default_key").and_then(|v| v.as_str()),
        Some("default_val")
    );
    assert_eq!(
        result.get("override_me").and_then(|v| v.as_str()),
        Some("new_val")
    );
}
