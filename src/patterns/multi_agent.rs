use crate::core::node::{Node, SharedStore};
use futures::future::join_all;
use std::pin::Pin;
use std::future::Future;

#[derive(Clone)]
pub enum MergeStrategy {
    SharedStore,
    Namespaced,
    Custom(fn(Vec<SharedStore>) -> SharedStore),
}

#[derive(Clone)]
/// Multi-agent coordination via shared store
pub struct MultiAgent {
    pub agents: Vec<Box<dyn Node<SharedStore, SharedStore>>>,
    pub strategy: MergeStrategy,
}

impl MultiAgent {
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            strategy: MergeStrategy::SharedStore,
        }
    }

    pub fn with_strategy(strategy: MergeStrategy) -> Self {
        Self {
            agents: Vec::new(),
            strategy,
        }
    }

    pub fn add_agent(&mut self, agent: Box<dyn Node<SharedStore, SharedStore>>) {
        self.agents.push(agent);
    }

    pub async fn run(&self, store: SharedStore) -> SharedStore {
        match &self.strategy {
            MergeStrategy::SharedStore => self.run_shared(store).await,
            MergeStrategy::Namespaced => self.run_namespaced(store).await,
            MergeStrategy::Custom(merge_fn) => self.run_custom(store, *merge_fn).await,
        }
    }

    async fn run_shared(&self, store: SharedStore) -> SharedStore {
        let futures = self.agents.iter().map(|agent| {
            // Each agent gets a (cheap) clone of the Arc, pointing to the same data.
            agent.call(store.clone())
        });

        // Wait for all agents to complete. They modify the store in place.
        join_all(futures).await;

        // Return the single, modified store.
        store
    }

    async fn run_namespaced(&self, store: SharedStore) -> SharedStore {
        let mut agent_stores = Vec::new();
        for (idx, agent) in self.agents.iter().enumerate() {
            let input = store.lock().await.clone();
            let agent_store = std::sync::Arc::new(tokio::sync::Mutex::new(input));
            let result = agent.call(agent_store).await;
            agent_stores.push((idx, result));
        }

        for (idx, agent_store) in agent_stores {
            let agent_data = agent_store.lock().await;
            let mut merged_store = store.lock().await;
            for (key, value) in agent_data.iter() {
                merged_store.insert(format!("agent_{}.{}", idx, key), value.clone());
            }
        }
        store
    }

    async fn run_custom(&self, store: SharedStore, merge_fn: fn(Vec<SharedStore>) -> SharedStore) -> SharedStore {
        let mut results = Vec::new();
        for agent in &self.agents {
            let input = store.lock().await.clone();
            let agent_store = std::sync::Arc::new(tokio::sync::Mutex::new(input));
            let result = agent.call(agent_store).await;
            results.push(result);
        }
        merge_fn(results)
    }
}

// The Node implementation for MultiAgent now reflects that it modifies a single store.
impl Node<SharedStore, SharedStore> for MultiAgent {
    fn call(&self, input: SharedStore) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
        Box::pin(self.run(input))
    }
}

impl Default for MultiAgent {
    fn default() -> Self {
        Self::new()
    }
}
