use crate::core::node::{Node, SharedStore};
use std::future::Future;
use std::pin::Pin;

#[derive(Clone)]
/// Minimal agent pattern - autonomously makes decisions
pub struct Agent<N> {
    node: N,
    pub max_retries: usize,
    pub wait_millis: u64,
}

impl<N> Agent<N> {
    pub fn new(node: N) -> Self {
        Self {
            node,
            max_retries: 1,
            wait_millis: 0,
        }
    }

    pub fn with_retry(node: N, max_retries: usize, wait_millis: u64) -> Self {
        Self {
            node,
            max_retries,
            wait_millis,
        }
    }

    pub async fn decide(
        &self,
        input: std::collections::HashMap<String, serde_json::Value>,
    ) -> std::collections::HashMap<String, serde_json::Value>
    where
        N: Node<SharedStore, SharedStore> + Clone,
    {
        let shared_store = std::sync::Arc::new(tokio::sync::Mutex::new(input));
        let mut result_store = None;
        for attempt in 0..self.max_retries {
            let res = self.node.call(shared_store.clone()).await;
            let has_error = {
                let store = res.lock().await;
                store.contains_key("error")
            };
            if !has_error {
                result_store = Some(res);
                break;
            }
            result_store = Some(res);
            if attempt < self.max_retries - 1 && self.wait_millis > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(self.wait_millis)).await;
            }
        }
        let result_store = result_store.unwrap_or(shared_store);
        std::sync::Arc::try_unwrap(result_store).map_or_else(
            |arc| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async { arc.lock().await.clone() })
            },
            |mutex| mutex.into_inner(),
        )
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
