use crate::core::error::AgentFlowError;
use crate::core::node::{Node, SharedStore, SimpleNode};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use tracing::{debug, info, instrument, warn};

/// A directed graph of [`SimpleNode`]s connected by labeled edges.
///
/// `Flow` is the core orchestration primitive. Nodes are vertices; edges are
/// labeled with *action strings* that nodes write into the `"action"` key of
/// the [`SharedStore`] to select the next node at runtime.
///
/// # Routing
///
/// After each node executes, `Flow` reads `store["action"]`. It looks up the
/// edge `(current_node, action) → next_node`. If no matching edge exists, or
/// `"action"` is absent, execution stops. The `"action"` key is removed from
/// the store when the flow completes.
///
/// # Cycle prevention
///
/// Use [`with_max_steps`](Self::with_max_steps) to cap the total number of node
/// executions. Choose [`run`](Self::run) (writes `"error"` key on limit) or
/// [`run_safe`](Self::run_safe) (returns `Err`) depending on whether you need
/// strict error propagation.
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::prelude::*;
/// use std::collections::HashMap;
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
///
/// #[tokio::main]
/// async fn main() {
///     let planner = create_node(|store: SharedStore| async move {
///         store.write().await.insert("action".into(), serde_json::json!("execute"));
///         store
///     });
///     let executor = create_node(|store: SharedStore| async move {
///         store.write().await.insert("done".into(), serde_json::json!(true));
///         store
///     });
///
///     let mut flow = Flow::new().with_max_steps(10);
///     flow.add_node("planner",  planner);
///     flow.add_node("executor", executor);
///     flow.add_edge("planner", "execute", "executor");
///
///     let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
///     let result = flow.run(store).await;
/// }
/// ```
pub struct Flow {
    nodes: HashMap<String, SimpleNode>,
    edges: HashMap<String, HashMap<String, String>>,
    start_node: Option<String>,
    /// Maximum number of node executions before the flow is forcibly stopped.
    /// `None` means unlimited (use with care in graphs that may cycle).
    pub max_steps: Option<usize>,
}

impl Flow {
    /// Create a new empty flow with no nodes or edges.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            start_node: None,
            max_steps: None,
        }
    }

    /// Set the maximum number of node executions to prevent infinite loops.
    ///
    /// When the limit is reached, [`run`](Self::run) writes `"error"` into the
    /// store and halts. [`run_safe`](Self::run_safe) returns
    /// `Err(AgentFlowError::ExecutionLimitExceeded)` instead.
    pub fn with_max_steps(mut self, limit: usize) -> Self {
        self.max_steps = Some(limit);
        self
    }

    /// Create a flow and immediately register `node` as both the first node
    /// and the start node.
    pub fn with_start(name: &str, node: SimpleNode) -> Self {
        let mut flow = Self::new();
        flow.add_node(name, node);
        flow
    }

    /// Register a node. The **first** node added becomes the start node.
    pub fn add_node(&mut self, name: &str, node: SimpleNode) {
        if self.start_node.is_none() {
            self.start_node = Some(name.to_string());
        }
        self.nodes.insert(name.to_string(), node);
        self.edges.insert(name.to_string(), HashMap::new());
    }

    /// Add a directed edge: when `from` emits `action`, transition to `to`.
    ///
    /// The special action `"default"` is used by [`Workflow::connect`] for
    /// unconditional transitions.
    ///
    /// [`Workflow::connect`]: crate::patterns::workflow::Workflow::connect
    pub fn add_edge(&mut self, from: &str, action: &str, to: &str) {
        self.edges
            .entry(from.to_string())
            .or_default()
            .insert(action.to_string(), to.to_string());
    }

    /// Shared execution logic for [`run`](Self::run) and [`run_safe`](Self::run_safe).
    ///
    /// `on_limit_exceeded` controls behavior when `max_steps` is reached:
    /// - `Ok(store)` path  → write `"error"` key and return (used by `run`)
    /// - `Err(…)` path     → return `Err(AgentFlowError::ExecutionLimitExceeded)` (used by `run_safe`)
    async fn run_internal(
        &self,
        mut store: SharedStore,
        safe: bool,
    ) -> Result<SharedStore, AgentFlowError> {
        let mut current_node_name = match self.start_node.as_deref() {
            Some(name) => name.to_string(),
            None => return Ok(store),
        };

        let mut steps = 0;
        let limit = self.max_steps.unwrap_or(usize::MAX);

        while let Some(node) = self.nodes.get(&current_node_name) {
            if steps >= limit {
                warn!(steps, limit, "Flow exceeded max_steps limit");
                if safe {
                    return Err(AgentFlowError::ExecutionLimitExceeded(
                        "Flow execution exceeded max_steps limit".to_string(),
                    ));
                } else {
                    store.write().await.insert(
                        "error".to_string(),
                        serde_json::Value::String(
                            "Flow execution exceeded max_steps limit".to_string(),
                        ),
                    );
                    break;
                }
            }
            steps += 1;
            debug!(step = steps, node = %current_node_name, "Flow executing node");
            // Drop span before await so the future remains Send
            drop(tracing::info_span!("flow.node", node = %current_node_name, step = steps).entered());
            store = node.call(store).await;

            // Use a read lock — we are only reading the "action" key
            let action = store
                .read()
                .await
                .get("action")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "default".to_string());

            debug!(step = steps, node = %current_node_name, action = %action, "Flow transition");

            if let Some(next_node) = self
                .edges
                .get(&current_node_name)
                .and_then(|edges| edges.get(&action))
            {
                current_node_name = next_node.clone();
            } else {
                break;
            }
        }

        store.write().await.remove("action");
        info!(total_steps = steps, "Flow run complete");
        Ok(store)
    }

    /// Execute the flow from the start node.
    ///
    /// On [`max_steps`](Self::max_steps) exceeded, inserts
    /// `"error" = "Flow execution exceeded max_steps limit"` into the store
    /// and returns. Use [`run_safe`](Self::run_safe) for strict error handling.
    #[instrument(name = "flow.run", skip(self, store), fields(start = self.start_node.as_deref().unwrap_or("none"), max_steps = self.max_steps))]
    pub async fn run(&self, store: SharedStore) -> SharedStore {
        // SAFETY: `safe = false` means the internal function always returns Ok(store).
        self.run_internal(store, false).await.unwrap()
    }

    /// Execute the flow, returning `Err(AgentFlowError::ExecutionLimitExceeded)`
    /// if [`max_steps`](Self::max_steps) is reached.
    ///
    /// Prefer this over [`run`](Self::run) when you need to distinguish between
    /// a natural flow completion and a runaway loop.
    ///
    /// # Errors
    ///
    /// Returns [`AgentFlowError::ExecutionLimitExceeded`] when the step limit
    /// is exceeded.
    #[instrument(name = "flow.run_safe", skip(self, store), fields(start = self.start_node.as_deref().unwrap_or("none"), max_steps = self.max_steps))]
    pub async fn run_safe(&self, store: SharedStore) -> Result<SharedStore, AgentFlowError> {
        self.run_internal(store, true).await
    }

    /// Look up a node by name. Returns `None` if not registered.
    pub fn get_node(&self, name: &str) -> Option<&SimpleNode> {
        self.nodes.get(name)
    }

    /// Returns the name of the node that `from` transitions to when `action` is emitted,
    /// or `None` if no such edge exists.
    pub fn get_next_step(&self, from: &str, action: &str) -> Option<String> {
        self.edges
            .get(from)
            .and_then(|edges| edges.get(action))
            .cloned()
    }
}

impl Node<SharedStore, SharedStore> for Flow {
    fn call(&self, input: SharedStore) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
        Box::pin(self.run(input))
    }
}

impl Clone for Flow {
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
        }
    }
}

impl Default for Flow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::create_node;

    #[tokio::test]
    async fn test_max_steps_prevents_infinite_loop() {
        let mut flow = Flow::new().with_max_steps(5);

        let node_a = create_node(|store| async move {
            {
                let mut guard = store.write().await;
                guard.insert("action".to_string(), serde_json::Value::String("to_b".to_string()));
            }
            store
        });

        let node_b = create_node(|store| async move {
            {
                let mut guard = store.write().await;
                guard.insert("action".to_string(), serde_json::Value::String("to_a".to_string()));
            }
            store
        });

        flow.add_node("A", node_a);
        flow.add_node("B", node_b);
        flow.add_edge("A", "to_b", "B");
        flow.add_edge("B", "to_a", "A");

        let store = HashMap::new();
        let shared_store = std::sync::Arc::new(tokio::sync::RwLock::new(store));

        let result_shared = flow.run(shared_store).await;
        let result = result_shared.write().await;

        assert!(result.contains_key("error"));
        let error_msg = result
            .get("error")
            .and_then(|v| v.as_str())
            .expect("error key should be a string");
        assert_eq!(error_msg, "Flow execution exceeded max_steps limit");
    }
}
