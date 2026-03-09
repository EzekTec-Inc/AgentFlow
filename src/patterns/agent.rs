use crate::core::error::AgentFlowError;
use crate::core::node::{Node, NodeResult, SharedStore};
use std::future::Future;
use std::pin::Pin;
use tracing::{debug, warn};

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

    pub async fn decide_shared(
        &self,
        shared_store: SharedStore,
    ) -> SharedStore
    where
        N: Node<SharedStore, SharedStore> + Clone,
    {
        let mut result_store = None;
        for attempt in 0..self.max_retries {
            debug!(attempt, max_retries = self.max_retries, "Agent::decide_shared attempt");
            let res = self.node.call(shared_store.clone()).await;
            let has_error = {
                let store = res.write().await;
                store.contains_key("error")
            };
            if !has_error {
                result_store = Some(res);
                break;
            }
            warn!(attempt, "Agent::decide_shared node returned error key; retrying");
            result_store = Some(res);
            if attempt < self.max_retries - 1 && self.wait_millis > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(self.wait_millis)).await;
            }
        }
        result_store.unwrap_or(shared_store)
    }

    pub async fn decide(
        &self,
        input: std::collections::HashMap<String, serde_json::Value>,
    ) -> std::collections::HashMap<String, serde_json::Value>
    where
        N: Node<SharedStore, SharedStore> + Clone,
    {
        let shared_store = std::sync::Arc::new(tokio::sync::RwLock::new(input));
        let result_store = self.decide_shared(shared_store).await;
        let final_data = result_store.write().await.clone();
        final_data
    }

    /// Retry loop for a `NodeResult`-based node.
    /// - `AgentFlowError::Timeout` is treated as transient: retried up to `max_retries`.
    /// - Any other `AgentFlowError` variant is treated as fatal: returned immediately.
    pub async fn decide_result<R>(&self, input: SharedStore, node: &R) -> Result<SharedStore, AgentFlowError>
    where
        R: NodeResult<SharedStore, SharedStore> + Clone,
    {
        let mut last_err = AgentFlowError::NodeFailure("No attempts made".to_string());
        for attempt in 0..self.max_retries {
            debug!(attempt, max_retries = self.max_retries, "Agent::decide_result attempt");
            match node.call(input.clone()).await {
                Ok(store) => {
                    debug!(attempt, "Agent::decide_result succeeded");
                    return Ok(store);
                }
                Err(AgentFlowError::Timeout(msg)) => {
                    warn!(attempt, error = %msg, "Agent::decide_result timeout; retrying");
                    last_err = AgentFlowError::Timeout(msg);
                    if attempt < self.max_retries - 1 && self.wait_millis > 0 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(self.wait_millis)).await;
                    }
                }
                Err(other) => {
                    warn!(attempt, error = %other, "Agent::decide_result fatal error; aborting");
                    return Err(other);
                }
            }
        }
        Err(last_err)
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
