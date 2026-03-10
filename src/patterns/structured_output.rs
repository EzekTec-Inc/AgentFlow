use crate::core::node::{Node, SharedStore};
use std::future::Future;
use std::pin::Pin;

/// Wraps a node and validates that its output conforms to a structured format.
///
/// `StructuredOutput` is a thin decorator that passes the store through the
/// inner node and — in a production integration — would validate the resulting
/// JSON against a schema before returning it. The current implementation
/// passes through without schema validation; wire in your own schema checker
/// inside the inner node or subclass this type.
///
/// # Typical usage
///
/// Wrap an LLM node that is prompted to return JSON. The store key conventions
/// are by user agreement (e.g. the inner node writes `store["output"]` as a
/// JSON object, and the caller reads it with `serde_json::from_value`).
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::prelude::*;
///
/// let llm_node = create_node(|store: SharedStore| async move {
///     // Call your LLM with a JSON-mode prompt here.
///     store.write().await.insert(
///         "output".into(),
///         serde_json::json!({"name": "Alice", "age": 30}),
///     );
///     store
/// });
///
/// let structured = StructuredOutput::new(llm_node);
/// ```
#[derive(Clone)]
pub struct StructuredOutput<N> {
    /// The inner node whose output will be validated.
    pub node: N,
}

impl<N> StructuredOutput<N> {
    /// Wrap `node` in a `StructuredOutput` decorator.
    pub fn new(node: N) -> Self {
        Self { node }
    }

    /// Execute the inner node and return the store, or an error string if the
    /// node call fails.
    pub async fn generate(&self, prompt: SharedStore) -> Result<SharedStore, String>
    where
        N: Node<SharedStore, SharedStore>,
    {
        let raw = self.node.call(prompt).await;
        Ok(raw)
    }
}

impl<N> Node<SharedStore, SharedStore> for StructuredOutput<N>
where
    N: Node<SharedStore, SharedStore> + Clone,
{
    fn call(&self, input: SharedStore) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
        Box::pin(async move {
            self.generate(input).await.unwrap_or_else(|_| {
                std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()))
            })
        })
    }
}
