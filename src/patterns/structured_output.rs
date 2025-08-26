use crate::core::node::{Node, SharedStore};
use std::pin::Pin;
use std::future::Future;

#[derive(Clone)]
/// StructuredOutput formats outputs consistently
pub struct StructuredOutput<N> {
    pub node: N,
}

impl<N> StructuredOutput<N> {
    pub fn new(node: N) -> Self {
        Self { node }
    }

    pub async fn generate(&self, prompt: SharedStore) -> Result<SharedStore, String>
    where
        N: Node<SharedStore, SharedStore>,
    {
        // The node call returns the same store `Arc`
        let raw = self.node.call(prompt).await;
        // In a real implementation, you'd lock and validate the contents against a JSON schema
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
                std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()))
            })
        })
    }
}
