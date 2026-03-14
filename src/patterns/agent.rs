use crate::core::error::AgentFlowError;
use crate::core::node::{Node, NodeResult, SharedStore};
use std::future::Future;
use std::pin::Pin;
use tracing::{debug, info, instrument, warn};

/// Autonomous async decision-making unit with optional retry logic.
///
/// `Agent` wraps any [`Node`] and adds:
///
/// - **Retry on error key** ([`decide_shared`]) — reruns the inner node up to
///   `max_retries` times if the output store contains an `"error"` key.
/// - **Typed retry** ([`decide_result`]) — works with [`NodeResult`] nodes,
///   distinguishing transient ([`AgentFlowError::Timeout`]) from fatal errors.
///
/// # Choosing a method
///
/// | Method | Node type | Error signal | Retry on |
/// |---|---|---|---|
/// | [`decide_shared`] | [`Node`] | `"error"` key in store | any `"error"` key |
/// | [`decide`] | [`Node`] | `"error"` key | any `"error"` key (plain `HashMap` convenience wrapper) |
/// | [`decide_result`] | [`NodeResult`] | `Err(AgentFlowError)` | `Timeout` only |
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::prelude::*;
///
/// #[tokio::main]
/// async fn main() {
///     let node = create_node(|store: SharedStore| async move {
///         store.write().await.insert("answer".into(), serde_json::json!(42));
///         store
///     });
///
///     // 3 retries, 200 ms between attempts
///     let agent = Agent::with_retry(node, 3, 200);
///
///     let store = std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
///     let result = agent.decide_shared(store).await;
/// }
/// ```
///
/// [`decide_shared`]: Agent::decide_shared
/// [`decide`]: Agent::decide
/// [`decide_result`]: Agent::decide_result
#[derive(Clone)]
pub struct Agent<N> {
    node: N,
    /// Maximum number of attempts before giving up.
    pub max_retries: usize,
    /// Milliseconds to wait between retry attempts.
    pub wait_millis: u64,
}

impl<N> Agent<N> {
    /// Create an agent with a single attempt and no wait.
    pub fn new(node: N) -> Self {
        Self {
            node,
            max_retries: 1,
            wait_millis: 0,
        }
    }

    /// Create an agent with explicit retry settings.
    ///
    /// - `max_retries` — total number of attempts (1 = no retries).
    /// - `wait_millis` — delay between attempts in milliseconds.
    pub fn with_retry(node: N, max_retries: usize, wait_millis: u64) -> Self {
        Self {
            node,
            max_retries,
            wait_millis,
        }
    }

    /// Run the inner node with retry, operating on a [`SharedStore`].
    ///
    /// Retries when the output store contains an `"error"` key. Returns the
    /// last store produced (with or without `"error"`) after all attempts are
    /// exhausted.
    #[instrument(name = "agent.decide_shared", skip(self, shared_store), fields(max_retries = self.max_retries))]
    pub async fn decide_shared(&self, shared_store: SharedStore) -> SharedStore
    where
        N: Node<SharedStore, SharedStore> + Clone,
    {
        let mut result_store = None;
        for attempt in 0..self.max_retries {
            debug!(
                attempt,
                max_retries = self.max_retries,
                "Agent::decide_shared attempt"
            );
            let res = self.node.call(shared_store.clone()).await;
            let has_error = {
                let store = res.read().await;
                store.contains_key("error")
            };
            if !has_error {
                info!(attempt, "Agent::decide_shared succeeded");
                result_store = Some(res);
                break;
            }
            warn!(
                attempt,
                "Agent::decide_shared node returned error key; retrying"
            );
            result_store = Some(res);
            if attempt < self.max_retries - 1 && self.wait_millis > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(self.wait_millis)).await;
            }
        }
        result_store.unwrap_or(shared_store)
    }

    /// Convenience wrapper around [`decide_shared`] that accepts and returns
    /// a plain `HashMap` instead of a [`SharedStore`].
    ///
    /// Useful for top-level callers that don't yet hold an `Arc<RwLock<…>>`.
    ///
    /// [`decide_shared`]: Agent::decide_shared
    #[instrument(name = "agent.decide", skip(self, input), fields(max_retries = self.max_retries))]
    pub async fn decide(
        &self,
        input: std::collections::HashMap<String, serde_json::Value>,
    ) -> std::collections::HashMap<String, serde_json::Value>
    where
        N: Node<SharedStore, SharedStore> + Clone,
    {
        let shared_store = std::sync::Arc::new(tokio::sync::RwLock::new(input));
        let result_store = self.decide_shared(shared_store).await;
        let final_data = result_store.read().await.clone();
        final_data
    }

    /// Run a [`NodeResult`] node with retry, distinguishing transient from fatal errors.
    ///
    /// - [`AgentFlowError::Timeout`] → **transient**: retried up to `max_retries` times.
    /// - Any other [`AgentFlowError`] variant → **fatal**: returned immediately,
    ///   no further retries.
    ///
    /// # Errors
    ///
    /// Returns the last [`AgentFlowError`] if all retries are exhausted or a
    /// fatal error is encountered.
    #[instrument(name = "agent.decide_result", skip(self, input, node), fields(max_retries = self.max_retries))]
    pub async fn decide_result<R>(
        &self,
        input: SharedStore,
        node: &R,
    ) -> Result<SharedStore, AgentFlowError>
    where
        R: NodeResult<SharedStore, SharedStore> + Clone,
    {
        let mut last_err = AgentFlowError::NodeFailure("No attempts made".to_string());
        for attempt in 0..self.max_retries {
            debug!(
                attempt,
                max_retries = self.max_retries,
                "Agent::decide_result attempt"
            );
            match node.call(input.clone()).await {
                Ok(store) => {
                    info!(attempt, "Agent::decide_result succeeded");
                    return Ok(store);
                }
                Err(AgentFlowError::Timeout(msg)) => {
                    warn!(attempt, error = %msg, "Agent::decide_result timeout; retrying");
                    last_err = AgentFlowError::Timeout(msg);
                    if attempt < self.max_retries - 1 && self.wait_millis > 0 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(self.wait_millis))
                            .await;
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
    N: Node<SharedStore, SharedStore> + Clone + Send + Sync + 'static,
{
    fn call(&self, input: SharedStore) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
        Box::pin(self.decide_shared(input))
    }
}
