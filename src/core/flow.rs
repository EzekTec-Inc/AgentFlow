use crate::core::error::AgentFlowError;
use crate::core::node::{Node, ResultNode, SharedStore, SimpleNode};
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
/// Asynchronous hook function type.
pub type FlowHookFn = std::sync::Arc<
    dyn Fn(
            &str,
            SharedStore,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = SharedStore> + Send + 'static>>
        + Send
        + Sync,
>;

/// A node inside a [`Flow`], which can be either infallible or fallible.
#[derive(Clone)]
pub enum FlowNode {
    /// An infallible node returning `SharedStore`.
    Simple(SimpleNode),
    /// A fallible node returning `Result<SharedStore, AgentFlowError>`.
    Result(ResultNode),
}

/// A directional graph orchestrator of modular [`Node`]s.
pub struct Flow {
    nodes: HashMap<String, FlowNode>,
    edges: HashMap<String, HashMap<String, String>>,
    start_node: Option<String>,
    /// Maximum number of node executions before the flow is forcibly stopped.
    /// `None` means unlimited (use with care in graphs that may cycle).
    pub max_steps: Option<usize>,
    /// Optional hook executed before every node.
    pub pre_node_hook: Option<FlowHookFn>,
    /// Optional hook executed after every node.
    pub post_node_hook: Option<FlowHookFn>,
}

impl Flow {
    /// Create a new empty flow with no nodes or edges.
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
    ///
    /// When the limit is reached, [`run`](Self::run) writes `"error"` into the
    /// store and halts. [`run_safe`](Self::run_safe) returns
    /// `Err(AgentFlowError::ExecutionLimitExceeded)` instead.
    pub fn with_max_steps(mut self, limit: usize) -> Self {
        self.max_steps = Some(limit);
        self
    }

    /// Set a hook that will be called before every node execution.
    pub fn with_pre_node_hook<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn(&str, SharedStore) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = SharedStore> + Send + 'static,
    {
        self.pre_node_hook = Some(std::sync::Arc::new(move |name, store| {
            Box::pin(hook(name, store))
        }));
        self
    }

    /// Set a hook that will be called after every node execution.
    pub fn with_post_node_hook<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn(&str, SharedStore) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = SharedStore> + Send + 'static,
    {
        self.post_node_hook = Some(std::sync::Arc::new(move |name, store| {
            Box::pin(hook(name, store))
        }));
        self
    }

    /// Convenience constructor: create a [`Flow`] with a single node already
    /// registered as the start node.
    ///
    /// This is exactly equivalent to:
    ///
    /// ```rust
    /// # use agentflow::core::flow::Flow;
    /// # use agentflow::core::node::{create_node, SharedStore};
    /// # let node = create_node(|store: SharedStore| async move { store });
    /// let mut flow = Flow::new();
    /// flow.add_node("my_node", node);
    /// // "my_node" is now the start node (first node added wins)
    /// ```
    ///
    /// **What it does NOT do:**
    /// - It does **not** add any edges — the node is registered but entirely
    ///   isolated until you call [`add_edge`](Self::add_edge).
    /// - It does **not** accept a [`ResultNode`](crate::core::node::ResultNode);
    ///   use [`add_result_node`](Self::add_result_node) for that variant.
    ///
    /// Prefer `with_start` for simple linear flows where the first node is
    /// known at construction time. For more complex graphs, use
    /// `Flow::new()` + repeated [`add_node`](Self::add_node) / [`add_edge`](Self::add_edge)
    /// calls, then optionally [`set_start`](Self::set_start) to pin the entry
    /// point explicitly.
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
        self.nodes.insert(name.to_string(), FlowNode::Simple(node));
        self.edges.entry(name.to_string()).or_default();
    }

    /// Register a fallible node. The **first** node added becomes the start node.
    pub fn add_result_node(&mut self, name: &str, node: crate::core::node::ResultNode) {
        if self.start_node.is_none() {
            self.start_node = Some(name.to_string());
        }
        self.nodes.insert(name.to_string(), FlowNode::Result(node));
        self.edges.entry(name.to_string()).or_default();
    }

    /// Explicitly set (or override) the start node.
    ///
    /// Use this when you need to guarantee which node runs first regardless of
    /// the order nodes were added via [`add_node`](Self::add_node).
    ///
    /// # Panics
    ///
    /// Does **not** panic if `name` is not yet registered — the node may be
    /// added later. `run` / `run_safe` will silently return an empty store if
    /// the start node name does not resolve at execution time.
    pub fn set_start(&mut self, name: &str) {
        self.start_node = Some(name.to_string());
    }

    /// Add a directed edge: when `from` emits `action`, transition to `to`.
    ///
    /// The special action `"default"` is used by [`Workflow::connect`] for
    /// unconditional transitions. It also acts as a **fallback**: if a node
    /// does not write `store["action"]` (or writes a non-string value),
    /// [`Flow::run`] resolves the action to `"default"` automatically.
    ///
    /// # Warning — silent advance on missing `"action"`
    ///
    /// If a node forgets to set `store["action"]` and a `"default"` edge
    /// exists for that node, execution will advance to the next node **without
    /// error or warning**. This can mask bugs where a conditional node was
    /// supposed to halt but silently continues instead. If you need strict
    /// halting on missing actions, do not register a `"default"` edge for
    /// nodes that must always set an explicit action.
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

            if let Some(hook) = &self.pre_node_hook {
                store = hook(&current_node_name, store).await;
            }

            // Drop span before await so the future remains Send
            drop(
                tracing::info_span!("flow.node", node = %current_node_name, step = steps).entered(),
            );
            println!("Executing node: {}", current_node_name);
            store = match node {
                FlowNode::Simple(n) => n.call(store).await,
                FlowNode::Result(n) => match n.call(store.clone()).await {
                    Ok(s) => s,
                    Err(e) => {
                        if safe {
                            return Err(e);
                        } else {
                            store.write().await.insert(
                                "error".to_string(),
                                serde_json::Value::String(e.to_string()),
                            );
                            break;
                        }
                    }
                },
            };

            if let Some(hook) = &self.post_node_hook {
                store = hook(&current_node_name, store).await;
            }

            // Consume the "action" key to route, preventing it from leaking to the next node
            let action = store
                .write()
                .await
                .remove("action")
                .and_then(|v| match v {
                    serde_json::Value::String(s) => Some(s),
                    _ => None,
                })
                .unwrap_or_else(|| "default".to_string());

            println!("Action: {}", action);

            debug!(step = steps, node = %current_node_name, action = %action, "Flow transition");

            if let Some(next_node) = self
                .edges
                .get(&current_node_name)
                .and_then(|edges| edges.get(&action))
            {
                println!("Next node: {}", next_node);
                current_node_name = next_node.clone();
            } else {
                println!("No next node, breaking");
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
        // When `safe = false`, `run_internal` always returns `Ok(store)`.
        match self.run_internal(store, false).await {
            Ok(s) => s,
            Err(_) => unreachable!("run_internal with safe=false never returns Err"),
        }
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
    pub fn get_node(&self, name: &str) -> Option<&FlowNode> {
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
            pre_node_hook: self.pre_node_hook.clone(),
            post_node_hook: self.post_node_hook.clone(),
        }
    }
}

impl Default for Flow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::core::node::create_node;

    #[tokio::test]
    async fn test_max_steps_prevents_infinite_loop() {
        let mut flow = Flow::new().with_max_steps(5);

        let node_a = create_node(|store| async move {
            {
                let mut guard = store.write().await;
                guard.insert(
                    "action".to_string(),
                    serde_json::Value::String("to_b".to_string()),
                );
            }
            store
        });

        let node_b = create_node(|store| async move {
            {
                let mut guard = store.write().await;
                guard.insert(
                    "action".to_string(),
                    serde_json::Value::String("to_a".to_string()),
                );
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
