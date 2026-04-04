use crate::core::error::AgentFlowError;
use crate::core::typed_store::TypedStore;
use dyn_clone::DynClone;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use tracing::{debug, instrument, warn};

/// Core Node trait for typed state and enum-based routing
pub trait TypedNode<T, E>: Send + Sync + DynClone {
    /// Consume `input`, run the node's logic, and return the mutated store
    /// along with an optional transition action.
    fn call(&self, input: TypedStore<T>) -> TypedNodeFuture<'_, T, E>;
}
dyn_clone::clone_trait_object!(<T, E> TypedNode<T, E>);

/// Boxed future returned by [`TypedNode::call`].
pub type TypedNodeFuture<'a, T, E> =
    Pin<Box<dyn Future<Output = (TypedStore<T>, Option<E>)> + Send + 'a>>;

/// Boxed, type-erased [`TypedNode`] used in a [`TypedFlow`].
pub type SimpleTypedNode<T, E> = Box<dyn TypedNode<T, E>>;

/// Helper to create a TypedNode from a function
pub fn create_typed_node<T, E, F, Fut>(func: F) -> SimpleTypedNode<T, E>
where
    T: Send + Sync + 'static,
    E: Send + Sync + Clone + 'static,
    F: Fn(TypedStore<T>) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = (TypedStore<T>, Option<E>)> + Send + 'static,
{
    struct FuncNode<T, E, F>(F, std::marker::PhantomData<(T, E)>);

    impl<T, E, F: Clone> Clone for FuncNode<T, E, F> {
        fn clone(&self) -> Self {
            FuncNode(self.0.clone(), std::marker::PhantomData)
        }
    }

    impl<T, E, F, Fut> TypedNode<T, E> for FuncNode<T, E, F>
    where
        T: Send + Sync + 'static,
        E: Send + Sync + Clone + 'static,
        F: Fn(TypedStore<T>) -> Fut + Send + Sync + Clone,
        Fut: Future<Output = (TypedStore<T>, Option<E>)> + Send + 'static,
    {
        fn call(&self, input: TypedStore<T>) -> TypedNodeFuture<'_, T, E> {
            Box::pin(self.0(input))
        }
    }

    Box::new(FuncNode(func, std::marker::PhantomData))
}

/// Asynchronous hook function type for `TypedFlow`.
pub type TypedFlowHookFn<T> = std::sync::Arc<
    dyn Fn(
            &str,
            TypedStore<T>,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = TypedStore<T>> + Send + 'static>>
        + Send
        + Sync,
>;

/// A flow orchestrator that strictly uses `TypedStore<T>` and enum-based transitions `E`.
pub struct TypedFlow<T, E> {
    nodes: HashMap<String, SimpleTypedNode<T, E>>,
    edges: HashMap<String, HashMap<E, String>>,
    start_node: Option<String>,
    /// Maximum number of node executions before the flow is forcibly stopped.
    /// `None` means unlimited (use with care in graphs that may cycle).
    pub max_steps: Option<usize>,
    /// Optional hook executed before every node.
    pub pre_node_hook: Option<TypedFlowHookFn<T>>,
    /// Optional hook executed after every node.
    pub post_node_hook: Option<TypedFlowHookFn<T>>,
}

impl<T, E> TypedFlow<T, E>
where
    E: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
{
    /// Create a new empty `TypedFlow` with no nodes or edges.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            start_node: None,
            max_steps: None,
            pre_node_hook: None,
            post_node_hook: None,
        }
    }

    /// Set the maximum number of node executions to prevent infinite loops.
    pub fn with_max_steps(mut self, limit: usize) -> Self {
        self.max_steps = Some(limit);
        self
    }

    /// Set a hook that will be called before every node execution.
    pub fn with_pre_node_hook<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn(&str, TypedStore<T>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = TypedStore<T>> + Send + 'static,
    {
        self.pre_node_hook = Some(std::sync::Arc::new(move |name, store| {
            Box::pin(hook(name, store))
        }));
        self
    }

    /// Set a hook that will be called after every node execution.
    pub fn with_post_node_hook<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn(&str, TypedStore<T>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = TypedStore<T>> + Send + 'static,
    {
        self.post_node_hook = Some(std::sync::Arc::new(move |name, store| {
            Box::pin(hook(name, store))
        }));
        self
    }

    /// Register a typed node. The **first** node added becomes the start node.
    pub fn add_node(&mut self, name: &str, node: SimpleTypedNode<T, E>) {
        if self.start_node.is_none() {
            self.start_node = Some(name.to_string());
        }
        self.nodes.insert(name.to_string(), node);
    }

    /// Add a directed edge: when `from` emits `action`, transition to `to`.
    pub fn add_edge(&mut self, from: &str, action: E, to: &str) {
        self.edges
            .entry(from.to_string())
            .or_default()
            .insert(action, to.to_string());
    }

    /// Execute the flow from the start node. On `max_steps` exceeded, sets
    /// `limit_exceeded` on the returned store and halts.
    #[instrument(name = "typed_flow.run", skip(self, store), fields(start = self.start_node.as_deref().unwrap_or("none"), max_steps = self.max_steps))]
    pub async fn run(&self, store: TypedStore<T>) -> TypedStore<T> {
        self.run_internal(store, false)
            .await
            .unwrap_or_else(|_| unreachable!())
    }

    /// Execute the flow from the start node. Returns
    /// `Err(AgentFlowError::ExecutionLimitExceeded)` if `max_steps` is reached.
    #[instrument(name = "typed_flow.run_safe", skip(self, store), fields(start = self.start_node.as_deref().unwrap_or("none"), max_steps = self.max_steps))]
    pub async fn run_safe(&self, store: TypedStore<T>) -> Result<TypedStore<T>, AgentFlowError> {
        self.run_internal(store, true).await
    }

    async fn run_internal(
        &self,
        store: TypedStore<T>,
        safe: bool,
    ) -> Result<TypedStore<T>, AgentFlowError> {
        let current_node_name = if let Some(name) = &self.start_node {
            name.clone()
        } else {
            return Ok(store);
        };

        let mut steps = 0;
        let limit = self.max_steps.unwrap_or(usize::MAX);

        let (tx, mut rx) = tokio::sync::mpsc::channel::<(TypedStore<T>, Option<E>, String)>(32);

        let _ = tx.send((store, None, current_node_name)).await;

        while let Some((mut current_store, action_opt, current_name)) = rx.recv().await {
            let next_node = if let Some(action) = action_opt {
                if let Some(next) = self.edges.get(&current_name).and_then(|e| e.get(&action)) {
                    next.clone()
                } else {
                    return Ok(current_store);
                }
            } else {
                current_name
            };

            if !self.nodes.contains_key(&next_node) {
                return Ok(current_store);
            }

            if steps >= limit {
                warn!(steps, limit, "TypedFlow exceeded max_steps limit");
                if safe {
                    return Err(AgentFlowError::ExecutionLimitExceeded(
                        "TypedFlow execution exceeded max_steps limit".to_string(),
                    ));
                } else {
                    current_store.limit_exceeded = true;
                    return Ok(current_store);
                }
            }
            steps += 1;
            debug!(step = steps, node = %next_node, "TypedFlow executing node");

            let node = match self.nodes.get(&next_node) {
                Some(n) => n,
                None => return Ok(current_store),
            };

            if let Some(hook) = &self.pre_node_hook {
                current_store = hook(&next_node, current_store).await;
            }

            let start_time = std::time::Instant::now();
            let (new_store, new_action_opt) = node.call(current_store).await;
            let elapsed = start_time.elapsed();

            current_store = new_store;
            current_store
                .context
                .record_node_duration(&next_node, elapsed);

            if let Some(hook) = &self.post_node_hook {
                current_store = hook(&next_node, current_store).await;
            }

            if new_action_opt.is_none() {
                return Ok(current_store);
            } else {
                let _ = tx.send((current_store, new_action_opt, next_node)).await;
            }
        }

        unreachable!("MPSC channel loop terminated unexpectedly")
    }
}

impl<T, E> Clone for TypedFlow<T, E>
where
    E: Clone,
{
    fn clone(&self) -> Self {
        let mut new_nodes = HashMap::new();
        for (k, v) in &self.nodes {
            new_nodes.insert(k.clone(), v.clone());
        }
        Self {
            nodes: new_nodes,
            edges: self.edges.clone(),
            start_node: self.start_node.clone(),
            max_steps: self.max_steps,
            pre_node_hook: self.pre_node_hook.clone(),
            post_node_hook: self.post_node_hook.clone(),
        }
    }
}

impl<T, E> Default for TypedFlow<T, E>
where
    E: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestState {
        count: u32,
        messages: Vec<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum TestAction {
        Next,
        Loop,
    }

    #[tokio::test]
    async fn test_typed_flow_execution() {
        let mut flow = TypedFlow::<TestState, TestAction>::new().with_max_steps(10);

        let node_a = create_typed_node(|mut store: TypedStore<TestState>| async move {
            let mut count = 0;
            store.inner.count += 1;
            count = store.inner.count;
            store.inner.messages.push(format!("A: {}", count));
            if count < 3 {
                (store, Some(TestAction::Next))
            } else {
                (store, None)
            }
        });

        let node_b = create_typed_node(|mut store: TypedStore<TestState>| async move {
            store.inner.count += 1;
            let c = store.inner.count;
            store.inner.messages.push(format!("B: {}", c));
            (store, Some(TestAction::Loop))
        });

        flow.add_node("A", node_a);
        flow.add_node("B", node_b);

        flow.add_edge("A", TestAction::Next, "B");
        flow.add_edge("B", TestAction::Loop, "A");

        let state = TestState {
            count: 0,
            messages: vec![],
        };
        let store = TypedStore::new(state);

        let final_store = flow.run(store).await;
        let final_state = final_store.inner;

        assert_eq!(final_state.count, 3);
        assert_eq!(final_state.messages, vec!["A: 1", "B: 2", "A: 3"]);
    }

    #[tokio::test]
    async fn test_typed_flow_max_steps_prevents_infinite_loop() {
        let mut flow = TypedFlow::<TestState, TestAction>::new().with_max_steps(5);

        let node_a = create_typed_node(|mut store: TypedStore<TestState>| async move {
            store.inner.count += 1;
            (store, Some(TestAction::Loop))
        });

        flow.add_node("A", node_a);
        flow.add_edge("A", TestAction::Loop, "A");

        let store = TypedStore::new(TestState {
            count: 0,
            messages: vec![],
        });
        let result = flow.run(store).await;
        let state = result.inner;
        assert_eq!(state.count, 5);
        assert!(result.limit_exceeded);
    }

    #[tokio::test]
    async fn test_typed_flow_run_safe_returns_error_on_limit() {
        let mut flow = TypedFlow::<TestState, TestAction>::new().with_max_steps(2);

        let node_a = create_typed_node(|store: TypedStore<TestState>| async move {
            (store, Some(TestAction::Loop))
        });

        flow.add_node("A", node_a);
        flow.add_edge("A", TestAction::Loop, "A");

        let store = TypedStore::new(TestState {
            count: 0,
            messages: vec![],
        });
        let result = flow.run_safe(store).await;
        assert!(matches!(
            result,
            Err(AgentFlowError::ExecutionLimitExceeded(_))
        ));
    }

    #[tokio::test]
    async fn test_typed_flow_run_sets_limit_exceeded_flag() {
        let mut flow = TypedFlow::<TestState, TestAction>::new().with_max_steps(3);

        let node_a = create_typed_node(|store: TypedStore<TestState>| async move {
            (store, Some(TestAction::Loop))
        });

        flow.add_node("A", node_a);
        flow.add_edge("A", TestAction::Loop, "A");

        let store = TypedStore::new(TestState {
            count: 0,
            messages: vec![],
        });
        let final_store = flow.run(store).await;
        assert!(final_store.limit_exceeded);
    }
}
