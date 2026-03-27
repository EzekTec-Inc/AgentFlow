use agentflow::core::error::AgentFlowError;
use agentflow::core::typed_flow::{create_typed_node, TypedFlow};
use agentflow::core::typed_store::TypedStore;

#[derive(Debug, Clone)]
struct MyState {
    pub step_a: bool,
    pub step_b: bool,
    pub count: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Action {
    Next,
    Loop,
}

#[tokio::test]
async fn test_typed_flow_linear() {
    let mut flow = TypedFlow::<MyState, Action>::new();

    let a = create_typed_node(|mut store: TypedStore<MyState>| async move {
        store.inner.step_a = true;
        (store, Some(Action::Next))
    });

    let b = create_typed_node(|mut store: TypedStore<MyState>| async move {
        store.inner.step_b = true;
        (store, None)
    });

    flow.add_node("A", a);
    flow.add_node("B", b);

    flow.add_edge("A", Action::Next, "B");

    let store = TypedStore::new(MyState {
        step_a: false,
        step_b: false,
        count: 0,
    });

    let result = flow.run(store).await;

    let state = result.inner;
    assert!(state.step_a);
    assert!(state.step_b);
}

#[tokio::test]
async fn test_typed_flow_run_safe_cycle() {
    let mut flow = TypedFlow::<MyState, Action>::new().with_max_steps(2);

    let node = create_typed_node(|mut store: TypedStore<MyState>| async move {
        store.inner.count += 1;
        (store, Some(Action::Loop))
    });

    flow.add_node("A", node);
    flow.add_edge("A", Action::Loop, "A");

    let store = TypedStore::new(MyState {
        step_a: false,
        step_b: false,
        count: 0,
    });

    let result = flow.run_safe(store).await;

    match result {
        Err(AgentFlowError::ExecutionLimitExceeded(_)) => {}
        _ => panic!("Expected limit exceeded error"),
    }
}
