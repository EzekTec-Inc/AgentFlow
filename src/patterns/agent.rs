use crate::core::node::{Node, SharedStore};
use std::pin::Pin;
use std::future::Future;

#[derive(Clone)]
/// Minimal agent pattern - autonomously makes decisions
pub struct Agent<N> {
    node: N,
    pub max_retries: usize,
    pub wait_millis: u64,
}

impl<N> Agent<N> {
    pub fn new(node: N) -> Self {
        Self { node, max_retries: 1, wait_millis: 0 }
    }

    pub fn with_retry(node: N, max_retries: usize, wait_millis: u64) -> Self {
        Self { node, max_retries, wait_millis }
    }

    /// Single decide method that works for all cases, with retry/fallback logic
    pub async fn decide(&self, input: std::collections::HashMap<String, serde_json::Value>) -> std::collections::HashMap<String, serde_json::Value>
    where
        N: Node<SharedStore, SharedStore> + Clone,
    {
        let shared_store = std::sync::Arc::new(std::sync::Mutex::new(input));
        let mut result_store = None;
        for _ in 0..self.max_retries {
            let res = self.node.call(shared_store.clone()).await;
            // If you want to check for error, you must encode error in the store
            // Here, we just break after first run, but you could check for error keys
            result_store = Some(res.clone());
            break;
        }
        let result_store = result_store.unwrap_or(shared_store);
        std::sync::Arc::try_unwrap(result_store)
            .map_or_else(|arc| arc.lock().unwrap().clone(), |mutex| mutex.into_inner().unwrap())
    }
}

impl<N> Node<SharedStore, SharedStore> for Agent<N>
where
    N: Node<SharedStore, SharedStore> + Clone,
{
    fn call(&self, input: SharedStore) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
        self.node.call(input)
    }
}
