use crate::core::node::{Node, SharedStore};
use std::pin::Pin;
use std::future::Future;

#[derive(Clone)]
/// RAG integrates data retrieval with generation
pub struct Rag<R, G> {
    pub retriever: R,
    pub generator: G,
}

impl<R, G> Rag<R, G> {
    pub fn new(retriever: R, generator: G) -> Self {
        Self { retriever, generator }
    }

    pub async fn ask(&self, query: SharedStore) -> SharedStore
    where
        R: Node<SharedStore, SharedStore>,
        G: Node<SharedStore, SharedStore>,
    {
        // The retriever and generator will operate on the same shared store.
        // We pass the Arc<Mutex<>> through the chain.
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
