use crate::core::error::AgentFlowError;
use crate::core::typed_store::TypedStore;
use dyn_clone::DynClone;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};

/// Core Node trait for typed state
pub trait TypedNode<T>: Send + Sync + DynClone {
    fn call(
        &self,
        input: TypedStore<T>,
    ) -> Pin<Box<dyn Future<Output = TypedStore<T>> + Send + '_>>;
}
dyn_clone::clone_trait_object!(<T> TypedNode<T>);

pub type SimpleTypedNode<T> = Box<dyn TypedNode<T>>;

/// Helper to create a TypedNode from a function
pub fn create_typed_node<T, F, Fut>(func: F) -> SimpleTypedNode<T>
where
    T: Send + Sync + 'static,
    F: Fn(TypedStore<T>) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = TypedStore<T>> + Send + 'static,
{
    struct FuncNode<T, F>(F, std::marker::PhantomData<T>);

    impl<T, F: Clone> Clone for FuncNode<T, F> {
        fn clone(&self) -> Self {
            FuncNode(self.0.clone(), std::marker::PhantomData)
        }
    }

    impl<T, F, Fut> TypedNode<T> for FuncNode<T, F>
    where
        T: Send + Sync + 'static,
        F: Fn(TypedStore<T>) -> Fut + Send + Sync + Clone,
        Fut: Future<Output = TypedStore<T>> + Send + 'static,
    {
        fn call(
            &self,
            input: TypedStore<T>,
        ) -> Pin<Box<dyn Future<Output = TypedStore<T>> + Send + '_>> {
            Box::pin(self.0(input))
        }
    }

    Box::new(FuncNode(func, std::marker::PhantomData))
}

pub type TransitionFn<T> = Arc<dyn Fn(&T) -> Option<String> + Send + Sync>;

/// A flow orchestrator that strictly uses `TypedStore<T>`
pub struct TypedFlow<T> {
    nodes: HashMap<String, SimpleTypedNode<T>>,
    transitions: HashMap<String, TransitionFn<T>>,
    start_node: Option<String>,
    pub max_steps: Option<usize>,
}

impl<T> TypedFlow<T> {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            transitions: HashMap::new(),
            start_node: None,
            max_steps: None,
        }
    }

    /// Set a maximum number of steps to prevent infinite loops
    pub fn with_max_steps(mut self, limit: usize) -> Self {
        self.max_steps = Some(limit);
        self
    }

    /// Add a typed node to the flow
    pub fn add_node(&mut self, name: &str, node: SimpleTypedNode<T>) {
        if self.start_node.is_none() {
            self.start_node = Some(name.to_string());
        }
        self.nodes.insert(name.to_string(), node);
    }

    /// Add a transition function. The closure is given read-access to `T`
    /// to determine the name of the next node. Returning `None` halts the flow.
    pub fn add_transition<F>(&mut self, from: &str, transition_fn: F)
    where
        F: Fn(&T) -> Option<String> + Send + Sync + 'static,
    {
        self.transitions
            .insert(from.to_string(), Arc::new(transition_fn));
    }

    /// Execute the typed flow.
    ///
    /// If `max_steps` is set and the limit is reached, execution stops silently
    /// and the current store is returned. Use [`run_safe`](Self::run_safe) to
    /// receive an explicit error instead.
    #[instrument(name = "typed_flow.run", skip(self, store), fields(start = self.start_node.as_deref().unwrap_or("none"), max_steps = self.max_steps))]
    pub async fn run(&self, mut store: TypedStore<T>) -> TypedStore<T> {
        let mut current_node_name = if let Some(name) = &self.start_node {
            name.clone()
        } else {
            return store;
        };

        let mut steps = 0;
        let limit = self.max_steps.unwrap_or(usize::MAX);

        while let Some(node) = self.nodes.get(&current_node_name) {
            if steps >= limit {
                warn!(steps, limit, "TypedFlow::run exceeded max_steps limit");
                break;
            }
            steps += 1;
            debug!(step = steps, node = %current_node_name, "TypedFlow::run executing node");

            store = node.call(store).await;

            let next_node = {
                let guard = store.inner.read().await;
                if let Some(transition_fn) = self.transitions.get(&current_node_name) {
                    transition_fn(&guard)
                } else {
                    None
                }
            };

            if let Some(next) = next_node {
                current_node_name = next;
            } else {
                break;
            }
        }

        info!(total_steps = steps, "TypedFlow::run complete");
        store
    }

    /// Execute the typed flow, returning `Err(AgentFlowError::ExecutionLimitExceeded)`
    /// if `max_steps` is reached, and `Ok(TypedStore<T>)` otherwise.
    #[instrument(name = "typed_flow.run_safe", skip(self, store), fields(start = self.start_node.as_deref().unwrap_or("none"), max_steps = self.max_steps))]
    pub async fn run_safe(
        &self,
        mut store: TypedStore<T>,
    ) -> Result<TypedStore<T>, AgentFlowError> {
        let mut current_node_name = if let Some(name) = &self.start_node {
            name.clone()
        } else {
            return Ok(store);
        };

        let mut steps = 0;
        let limit = self.max_steps.unwrap_or(usize::MAX);

        while let Some(node) = self.nodes.get(&current_node_name) {
            if steps >= limit {
                warn!(steps, limit, "TypedFlow::run_safe exceeded max_steps limit");
                return Err(AgentFlowError::ExecutionLimitExceeded(
                    "TypedFlow execution exceeded max_steps limit".to_string(),
                ));
            }
            steps += 1;
            debug!(step = steps, node = %current_node_name, "TypedFlow::run_safe executing node");

            store = node.call(store).await;

            let next_node = {
                let guard = store.inner.read().await;
                if let Some(transition_fn) = self.transitions.get(&current_node_name) {
                    transition_fn(&guard)
                } else {
                    None
                }
            };

            if let Some(next) = next_node {
                current_node_name = next;
            } else {
                break;
            }
        }

        info!(total_steps = steps, "TypedFlow::run_safe complete");
        Ok(store)
    }
}

impl<T> Clone for TypedFlow<T> {
    fn clone(&self) -> Self {
        let mut new_nodes = HashMap::new();
        for (k, v) in &self.nodes {
            new_nodes.insert(k.clone(), v.clone());
        }
        Self {
            nodes: new_nodes,
            transitions: self.transitions.clone(),
            start_node: self.start_node.clone(),
            max_steps: self.max_steps,
        }
    }
}

impl<T> Default for TypedFlow<T> {
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

    #[tokio::test]
    async fn test_typed_flow_execution() {
        let mut flow = TypedFlow::<TestState>::new().with_max_steps(10);

        let node_a = create_typed_node(|store: TypedStore<TestState>| async move {
            {
                let mut guard = store.inner.write().await;
                guard.count += 1;
                let c = guard.count;
                guard.messages.push(format!("A: {}", c));
            }
            store
        });

        let node_b = create_typed_node(|store: TypedStore<TestState>| async move {
            {
                let mut guard = store.inner.write().await;
                guard.count += 1;
                let c = guard.count;
                guard.messages.push(format!("B: {}", c));
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

        flow.add_transition("B", |_state| Some("A".to_string()));

        let state = TestState {
            count: 0,
            messages: vec![],
        };
        let store = TypedStore::new(state);

        let final_store = flow.run(store).await;
        let final_state = final_store.inner.read().await;

        assert_eq!(final_state.count, 3);
        assert_eq!(final_state.messages, vec!["A: 1", "B: 2", "A: 3"]);
    }

    #[tokio::test]
    async fn test_typed_flow_max_steps_prevents_infinite_loop() {
        let mut flow = TypedFlow::<TestState>::new().with_max_steps(5);

        let node_a = create_typed_node(|store: TypedStore<TestState>| async move {
            {
                let mut guard = store.inner.write().await;
                guard.count += 1;
            }
            store
        });

        let node_b = create_typed_node(|store: TypedStore<TestState>| async move {
            {
                let mut guard = store.inner.write().await;
                guard.count += 1;
            }
            store
        });

        flow.add_node("A", node_a);
        flow.add_node("B", node_b);

        // Infinite loop
        flow.add_transition("A", |_state| Some("B".to_string()));
        flow.add_transition("B", |_state| Some("A".to_string()));

        let state = TestState {
            count: 0,
            messages: vec![],
        };
        let store = TypedStore::new(state);

        let final_store = flow.run(store).await;
        let final_state = final_store.inner.read().await;

        // Loop stops after 5 steps
        assert_eq!(final_state.count, 5);
    }

    #[tokio::test]
    async fn test_typed_flow_run_safe_returns_error_on_limit() {
        let mut flow = TypedFlow::<TestState>::new().with_max_steps(3);

        let node_a = create_typed_node(|store: TypedStore<TestState>| async move {
            store.inner.write().await.count += 1;
            store
        });

        let node_b = create_typed_node(|store: TypedStore<TestState>| async move {
            store.inner.write().await.count += 1;
            store
        });

        flow.add_node("A", node_a);
        flow.add_node("B", node_b);

        // Infinite loop
        flow.add_transition("A", |_state| Some("B".to_string()));
        flow.add_transition("B", |_state| Some("A".to_string()));

        let store = TypedStore::new(TestState {
            count: 0,
            messages: vec![],
        });

        let result = flow.run_safe(store).await;
        assert!(
            matches!(result, Err(AgentFlowError::ExecutionLimitExceeded(_))),
            "expected ExecutionLimitExceeded, got {:?}",
            result
        );
    }
}
