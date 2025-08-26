use crate::core::node::{Node, SharedStore};
use futures::future::join_all;
use std::pin::Pin;
use std::future::Future;

#[derive(Clone)]
/// Multi-agent coordination via shared store
pub struct MultiAgent {
    pub agents: Vec<Box<dyn Node<SharedStore, SharedStore>>>,
}

impl MultiAgent {
    pub fn new() -> Self {
        Self { agents: Vec::new() }
    }

    pub fn add_agent(&mut self, agent: Box<dyn Node<SharedStore, SharedStore>>) {
        self.agents.push(agent);
    }

    pub async fn run(&self, store: SharedStore) -> SharedStore {
        let futures = self.agents.iter().map(|agent| {
            // Each agent gets a (cheap) clone of the Arc, pointing to the same data.
            agent.call(store.clone())
        });

        // Wait for all agents to complete. They modify the store in place.
        join_all(futures).await;

        // Return the single, modified store.
        store
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
