use crate::core::node::{Node, SharedStore, SimpleNode};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

/// Flow connects nodes through Actions (labeled edges)
pub struct Flow {
    nodes: HashMap<String, SimpleNode>,
    edges: HashMap<String, HashMap<String, String>>, // from_node -> action -> to_node
    start_node: Option<String>,
    pub max_steps: Option<usize>,
}

impl Flow {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            start_node: None,
            max_steps: None,
        }
    }

    /// Set a maximum number of steps to prevent infinite loops
    pub fn with_max_steps(mut self, limit: usize) -> Self {
        self.max_steps = Some(limit);
        self
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

        let mut steps = 0;
        let limit = self.max_steps.unwrap_or(usize::MAX);

        while let Some(node) = self.nodes.get(&current_node_name) {
            if steps >= limit {
                store.write().await.insert(
                    "error".to_string(),
                    serde_json::Value::String("Flow execution exceeded max_steps limit".to_string()),
                );
                break;
            }
            steps += 1;

            store = node.call(store).await;

            let action = {
                let guard = store.write().await;
                guard
                    .get("action")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "default".to_string())
            };

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
        assert_eq!(
            result.get("error").unwrap().as_str().unwrap(),
            "Flow execution exceeded max_steps limit"
        );
    }
}
