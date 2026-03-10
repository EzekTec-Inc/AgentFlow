use crate::core::node::{Node, SharedStore};
use std::future::Future;
use std::pin::Pin;

/// Retrieval-Augmented Generation pipeline.
///
/// `Rag` composes two nodes in sequence:
///
/// 1. **Retriever** — reads a query (e.g. `store["query"]`), fetches relevant
///    context from a database or search engine, and writes it back (e.g.
///    `store["context"]`).
/// 2. **Generator** — reads the context produced by the retriever and generates
///    a final response (e.g. writes `store["response"]`).
///
/// Both nodes operate on the same [`SharedStore`] — the retriever enriches it
/// in place, then the generator reads from it.
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
///     let retriever = create_node(|store: SharedStore| async move {
///         let query = store.read().await.get("query")
///             .and_then(|v| v.as_str()).unwrap_or("").to_string();
///         store.write().await.insert("context".into(), serde_json::json!(format!("docs for: {query}")));
///         store
///     });
///
///     let generator = create_node(|store: SharedStore| async move {
///         let ctx = store.read().await.get("context")
///             .and_then(|v| v.as_str()).unwrap_or("").to_string();
///         store.write().await.insert("response".into(), serde_json::json!(format!("answer based on: {ctx}")));
///         store
///     });
///
///     let rag = Rag::new(retriever, generator);
///
///     let mut init = HashMap::new();
///     init.insert("query".into(), serde_json::json!("Rust async runtimes"));
///     let store: SharedStore = Arc::new(RwLock::new(init));
///
///     let result = rag.call(store).await;
/// }
/// ```
#[derive(Clone)]
pub struct Rag<R, G> {
    /// The retrieval node. Reads `store["query"]`, writes `store["context"]`.
    pub retriever: R,
    /// The generation node. Reads `store["context"]`, writes `store["response"]`.
    pub generator: G,
}

impl<R, G> Rag<R, G> {
    /// Create a new RAG pipeline from a retriever and a generator node.
    pub fn new(retriever: R, generator: G) -> Self {
        Self {
            retriever,
            generator,
        }
    }

    /// Execute the pipeline: retriever → generator.
    pub async fn ask(&self, query: SharedStore) -> SharedStore
    where
        R: Node<SharedStore, SharedStore>,
        G: Node<SharedStore, SharedStore>,
    {
        let store_after_retrieval = self.retriever.call(query).await;
        self.generator.call(store_after_retrieval).await
    }
}

impl<R, G> Node<SharedStore, SharedStore> for Rag<R, G>
where
    R: Node<SharedStore, SharedStore> + Clone,
    G: Node<SharedStore, SharedStore> + Clone,
{
    fn call(&self, input: SharedStore) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
        Box::pin(self.ask(input))
    }
}
