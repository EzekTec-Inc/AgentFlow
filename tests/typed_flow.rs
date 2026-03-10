use agentflow::core::error::AgentFlowError;
use agentflow::core::typed_flow::{create_typed_node, TypedFlow};
use agentflow::core::typed_store::TypedStore;

#[derive(Debug, Clone)]
struct MyState {
    pub step_a: bool,
    pub step_b: bool,
    pub count: i32,
}

#[tokio::test]
async fn test_typed_flow_linear() {
    let mut flow = TypedFlow::<MyState>::new();

    let a = create_typed_node(|store: TypedStore<MyState>| async move {
        store.inner.write().await.step_a = true;
        store
    });

    let b = create_typed_node(|store: TypedStore<MyState>| async move {
        store.inner.write().await.step_b = true;
        store
    });

    flow.add_node("A", a);
    flow.add_node("B", b);
    
    flow.add_transition("A", |state| {
        if state.step_a {
            Some("B".to_string())
        } else {
            None
        }
    });

    let store = TypedStore::new(MyState {
        step_a: false,
        step_b: false,
        count: 0,
    });
    
    let result = flow.run(store).await;
    
    let state = result.inner.read().await;
    assert!(state.step_a);
    assert!(state.step_b);
}

#[tokio::test]
async fn test_typed_flow_run_safe_cycle() {
    let mut flow = TypedFlow::<MyState>::new().with_max_steps(2);

    let node = create_typed_node(|store: TypedStore<MyState>| async move {
        store.inner.write().await.count += 1;
        store
    });

    flow.add_node("A", node);
    flow.add_transition("A", |_| Some("A".to_string()));

    let store = TypedStore::new(MyState {
        step_a: false,
        step_b: false,
        count: 0,
    });
    
    let result = flow.run_safe(store).await;
    
    match result {
        Err(AgentFlowError::ExecutionLimitExceeded(_)) => {
            // Check final state if needed
            // let final_store = result.unwrap_err().
            // How to get the store out? The error type doesn't hold it.
            // For now, just check the error type is correct.
        }
        _ => panic!("Expected limit exceeded error"),
    }
}
