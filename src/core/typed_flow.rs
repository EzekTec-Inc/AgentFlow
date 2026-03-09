use crate::core::typed_store::TypedStore;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use dyn_clone::DynClone;

/// Core Node trait for typed state
pub trait TypedNode<T>: Send + Sync + DynClone {
    fn call(&self, input: TypedStore<T>) -> Pin<Box<dyn Future<Output = TypedStore<T>> + Send + '_>>;
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
        fn call(&self, input: TypedStore<T>) -> Pin<Box<dyn Future<Output = TypedStore<T>> + Send + '_>> {
            Box::pin(self.0(input))
        }
    }

    Box::new(FuncNode(func, std::marker::PhantomData))
}

/// A flow orchestrator that strictly uses `TypedStore<T>`
pub struct TypedFlow<T> {
    nodes: HashMap<String, SimpleTypedNode<T>>,
    transitions: HashMap<String, Arc<dyn Fn(&T) -> Option<String> + Send + Sync>>,
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
        self.transitions.insert(from.to_string(), Arc::new(transition_fn));
    }

    /// Execute the typed flow
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
                break;
            }
            steps += 1;

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

        store
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
