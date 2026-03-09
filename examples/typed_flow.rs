use agentflow::core::{TypedFlow, TypedStore, create_typed_node};

#[derive(Debug, Clone)]
struct MyState {
    count: u32,
    messages: Vec<String>,
}

#[tokio::main]
async fn main() {
    let mut flow = TypedFlow::<MyState>::new().with_max_steps(10);

    let node_a = create_typed_node(|store: TypedStore<MyState>| async move {
        {
            let mut guard = store.inner.write().await;
            guard.count += 1;
            let c = guard.count;
            guard.messages.push(format!("Node A executed, count is now {}", c));
        }
        store
    });

    let node_b = create_typed_node(|store: TypedStore<MyState>| async move {
        {
            let mut guard = store.inner.write().await;
            guard.count += 1;
            let c = guard.count;
            guard.messages.push(format!("Node B executed, count is now {}", c));
        }
        store
    });

    flow.add_node("A", node_a);
    flow.add_node("B", node_b);

    flow.add_transition("A", |state| {
        if state.count < 3 {
            Some("B".to_string())
        } else {
            None
        }
    });

    flow.add_transition("B", |_state| {
        Some("A".to_string())
    });

    let state = MyState {
        count: 0,
        messages: vec![],
    };

    let store = TypedStore::new(state);
    let final_store = flow.run(store).await;

    let final_state = final_store.inner.read().await;
    println!("Final Count: {}", final_state.count);
    for msg in &final_state.messages {
        println!(" - {}", msg);
    }
}
