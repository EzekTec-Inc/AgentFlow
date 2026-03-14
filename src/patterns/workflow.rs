use crate::core::flow::Flow;
use crate::core::node::{Node, SharedStore, SimpleNode};
use std::future::Future;
use std::pin::Pin;
use std::time::Instant;
use tracing::{debug, info, instrument};

/// Linear sequence of named nodes with conditional branching.
///
/// `Workflow` is a thin builder on top of [`Flow`]. It is the right choice when
/// your pipeline is mostly sequential — you connect steps with
/// [`connect`](Self::connect) (uses the `"default"` action) and only add
/// conditional edges where the path needs to diverge.
///
/// For full graph control (loops, multiple named edges per node) use [`Flow`]
/// directly.
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::prelude::*;
/// use std::collections::HashMap;
///
/// #[tokio::main]
/// async fn main() {
///     let mut wf = Workflow::new();
///
///     wf.add_step("research", create_node(|store: SharedStore| async move {
///         store.write().await.insert("research".into(), serde_json::json!("findings"));
///         store
///     }));
///     wf.add_step("write", create_node(|store: SharedStore| async move {
///         store.write().await.insert("draft".into(), serde_json::json!("text"));
///         store
///     }));
///
///     wf.connect("research", "write");
///
///     let result = wf.execute_shared(HashMap::new()).await;
/// }
/// ```
///
/// [`Flow`]: crate::core::flow::Flow
pub struct Workflow {
    flow: Flow,
    /// Optional default parameters merged into the store at execution time.
    /// Keys already present in the caller's store are **not** overwritten.
    pub params: std::collections::HashMap<String, serde_json::Value>,
}

impl Workflow {
    /// Create an empty workflow with no steps.
    pub fn new() -> Self {
        Self {
            flow: Flow::new(),
            params: std::collections::HashMap::new(),
        }
    }

    /// Create a workflow and register `node` as the first (start) step.
    pub fn with_start(name: &str, node: SimpleNode) -> Self {
        Self {
            flow: Flow::with_start(name, node),
            params: std::collections::HashMap::new(),
        }
    }

    /// Register a step node. The first step added becomes the start node.
    pub fn add_step(&mut self, name: &str, node: SimpleNode) {
        self.flow.add_node(name, node);
    }

    /// Connect `from` → `to` using the `"default"` action (unconditional).
    ///
    /// This registers a `"default"` edge via [`Flow::add_edge`]. Because
    /// [`Flow::run`] falls back to `"default"` whenever a node does **not**
    /// write `store["action"]`, this edge will also fire if the node at `from`
    /// forgets to set an explicit action.
    ///
    /// # Warning — silent advance on missing `"action"`
    ///
    /// If a node registered with `connect` was intended to emit a named action
    /// on some code path, failing to set `store["action"]` on that path will
    /// silently advance to `to` instead of halting. To prevent this, use
    /// [`connect_with_action`](Self::connect_with_action) for nodes that must
    /// always produce an explicit routing decision, and do **not** add a
    /// `"default"` fallback edge for those nodes.
    pub fn connect(&mut self, from: &str, to: &str) {
        self.flow.add_edge(from, "default", to);
    }

    /// Connect `from` → `to` using a named action for conditional branching.
    ///
    /// The node at `from` must write `store["action"] = action` to trigger
    /// this edge.
    pub fn connect_with_action(&mut self, from: &str, action: &str, to: &str) {
        self.flow.add_edge(from, action, to);
    }

    /// Set default parameters that are merged into the store before execution.
    ///
    /// Parameters whose keys already exist in the caller's store are ignored
    /// (no overwrite).
    pub fn set_params(&mut self, params: std::collections::HashMap<String, serde_json::Value>) {
        self.params = params;
    }

    /// Execute the workflow, returning a [`SharedStore`].
    ///
    /// `params` (if any) are merged into `store` before the first step runs.
    /// This is the preferred method — it avoids an extra `Arc` round-trip
    /// compared to [`execute`](Self::execute).
    #[instrument(name = "workflow.execute_shared", skip(self, store))]
    pub async fn execute_shared(
        &self,
        mut store: std::collections::HashMap<String, serde_json::Value>,
    ) -> SharedStore {
        let t = Instant::now();
        debug!("Workflow::execute_shared merging params");
        for (k, v) in &self.params {
            store.entry(k.clone()).or_insert(v.clone());
        }
        let shared_store = std::sync::Arc::new(tokio::sync::RwLock::new(store));
        let result = self.flow.run(shared_store).await;
        info!(
            elapsed_ms = t.elapsed().as_millis(),
            "Workflow::execute_shared complete"
        );
        result
    }

    /// Execute the workflow, returning a plain `HashMap`.
    ///
    /// This is a convenience wrapper around [`execute_shared`](Self::execute_shared)
    /// for callers that don't need to pass the store on to other nodes.
    #[instrument(name = "workflow.execute", skip(self, store))]
    pub async fn execute(
        &self,
        store: std::collections::HashMap<String, serde_json::Value>,
    ) -> std::collections::HashMap<String, serde_json::Value> {
        let t = Instant::now();
        debug!("Workflow::execute starting");
        let result_store = self.execute_shared(store).await;
        let final_data = result_store.write().await.clone();
        info!(
            elapsed_ms = t.elapsed().as_millis(),
            "Workflow::execute complete"
        );
        final_data
    }

    /// Look up a step node by name.
    pub fn get_node(&self, name: &str) -> Option<&SimpleNode> {
        self.flow.get_node(name)
    }

    /// Return the name of the step that follows `from` when `action` is emitted,
    /// or `None` if no such edge exists.
    pub fn get_next_step(&self, from: &str, action: &str) -> Option<String> {
        self.flow.get_next_step(from, action)
    }
}

impl Clone for Workflow {
    fn clone(&self) -> Self {
        Self {
            flow: self.flow.clone(),
            params: self.params.clone(),
        }
    }
}

impl Node<SharedStore, SharedStore> for Workflow {
    fn call(&self, input: SharedStore) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
        let params = self.params.clone();
        Box::pin(async move {
            let t = Instant::now();
            debug!("Workflow::call (Node impl) merging params and running");
            {
                let mut store = input.write().await;
                for (k, v) in &params {
                    store.entry(k.clone()).or_insert(v.clone());
                }
            }
            let result = self.flow.run(input).await;
            info!(
                elapsed_ms = t.elapsed().as_millis(),
                "Workflow::call (Node impl) complete"
            );
            result
        })
    }
}

impl Default for Workflow {
    fn default() -> Self {
        Self::new()
    }
}
