use crate::core::node::{Node, SharedStore, SimpleNode};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

/// Flow connects nodes through Actions (labeled edges)
pub struct Flow {
    nodes: HashMap<String, SimpleNode>,
    edges: HashMap<String, HashMap<String, String>>, // from_node -> action -> to_node
    start_node: Option<String>,
}

impl Flow {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            start_node: None,
        }
    }

    /// Create a flow with a start node
    pub fn with_start(name: &str, node: SimpleNode) -> Self {
        let mut flow = Self::new();
        flow.add_node(name, node);
        flow
    }

    /// Add a node to the flow
    pub fn add_node(&mut self, name: &str, node: SimpleNode) {
        if self.start_node.is_none() {
            self.start_node = Some(name.to_string());
        }
        self.nodes.insert(name.to_string(), node);
        self.edges.insert(name.to_string(), HashMap::new());
    }

    /// Connect nodes with an action (labeled edge)
    pub fn add_edge(&mut self, from: &str, action: &str, to: &str) {
        self.edges
            .entry(from.to_string())
            //.or_insert_with(HashMap::new)
            .or_default()
            .insert(action.to_string(), to.to_string());
    }

    /// Execute the flow
    pub async fn run(&self, mut store: SharedStore) -> SharedStore {
        let mut current_node_name = if let Some(name) = &self.start_node {
            name.clone()
        } else {
            return store;
        };

        while let Some(node) = self.nodes.get(&current_node_name) {
            store = node.call(store).await;

            // Determine next node based on action
            let action = store
                .lock()
                .unwrap()
                .get("action")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "default".to_string());

            if let Some(next_node) = self.edges.get(&current_node_name).and_then(|edges| edges.get(&action)) {
                current_node_name = next_node.clone();
            } else {
                // No more edges for this action, flow is complete.
                break;
            }
        }

        // Remove action from final store
        store.lock().unwrap().remove("action");
        store
    }

    pub fn get_node(&self, name: &str) -> Option<&SimpleNode> {
        self.nodes.get(name)
    }

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

// Implement Clone for Flow by requiring that SimpleNode is Clone
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
        }
    }
}

impl Default for Flow {
    fn default() -> Self {
        Self::new()
    }
}
