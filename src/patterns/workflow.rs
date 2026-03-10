use crate::core::flow::Flow;
use crate::core::node::{Node, SharedStore, SimpleNode};
use std::future::Future;
use std::pin::Pin;

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
    pub async fn execute_shared(
        &self,
        mut store: std::collections::HashMap<String, serde_json::Value>,
    ) -> SharedStore {
        for (k, v) in &self.params {
            store.entry(k.clone()).or_insert(v.clone());
        }
        let shared_store = std::sync::Arc::new(tokio::sync::RwLock::new(store));
        self.flow.run(shared_store).await
    }

    /// Execute the workflow, returning a plain `HashMap`.
    ///
    /// This is a convenience wrapper around [`execute_shared`](Self::execute_shared)
    /// for callers that don't need to pass the store on to other nodes.
    pub async fn execute(
        &self,
        store: std::collections::HashMap<String, serde_json::Value>,
    ) -> std::collections::HashMap<String, serde_json::Value> {
        let result_store = self.execute_shared(store).await;
        let final_data = result_store.write().await.clone();
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
            {
                let mut store = input.write().await;
                for (k, v) in &params {
                    store.entry(k.clone()).or_insert(v.clone());
                }
            }
            self.flow.run(input).await
        })
    }
}

impl Default for Workflow {
    fn default() -> Self {
        Self::new()
    }
}
