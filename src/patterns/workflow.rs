use crate::core::flow::Flow;
use crate::core::node::{Node, SharedStore, SimpleNode};
use std::future::Future;
use std::pin::Pin;

/// Workflow chains multiple tasks into pipelines
pub struct Workflow {
    flow: Flow,
    pub params: std::collections::HashMap<String, serde_json::Value>,
}

impl Workflow {
    pub fn new() -> Self {
        Self {
            flow: Flow::new(),
            params: std::collections::HashMap::new(),
        }
    }

    /// Create workflow with a starting step
    pub fn with_start(name: &str, node: SimpleNode) -> Self {
        Self {
            flow: Flow::with_start(name, node),
            params: std::collections::HashMap::new(),
        }
    }

    /// Add a step to the workflow
    pub fn add_step(&mut self, name: &str, node: SimpleNode) {
        self.flow.add_node(name, node);
    }

    /// Connect steps with default action
    pub fn connect(&mut self, from: &str, to: &str) {
        self.flow.add_edge(from, "default", to);
    }

    /// Connect steps with specific action
    pub fn connect_with_action(&mut self, from: &str, action: &str, to: &str) {
        self.flow.add_edge(from, action, to);
    }

    /// Set workflow params (for parity with Python)
    pub fn set_params(&mut self, params: std::collections::HashMap<String, serde_json::Value>) {
        self.params = params;
    }

    /// Execute the workflow and return a SharedStore
    pub async fn execute_shared(
        &self,
        mut store: std::collections::HashMap<String, serde_json::Value>,
    ) -> SharedStore {
        // Merge params into store for parity with Python
        for (k, v) in &self.params {
            store.entry(k.clone()).or_insert(v.clone());
        }
        let shared_store = std::sync::Arc::new(tokio::sync::Mutex::new(store));
        self.flow.run(shared_store).await
    }

    /// Execute the workflow
    pub async fn execute(
        &self,
        store: std::collections::HashMap<String, serde_json::Value>,
    ) -> std::collections::HashMap<String, serde_json::Value> {
        let result_store = self.execute_shared(store).await;
        let final_data = result_store.lock().await.clone();
        final_data
    }

    /// Get a node by step name
    pub fn get_node(&self, name: &str) -> Option<&SimpleNode> {
        self.flow.get_node(name)
    }

    /// Get the next step for a given step and action
    pub fn get_next_step(&self, from: &str, action: &str) -> Option<String> {
        self.flow.get_next_step(from, action)
    }
}

// Implement Clone for Workflow (requires Flow: Clone)
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
                let mut store = input.lock().await;
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
