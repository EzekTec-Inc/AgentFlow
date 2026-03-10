use crate::core::node::{Node, SharedStore};
use futures::stream::StreamExt;
use std::future::Future;
use std::pin::Pin;

/// Processes a `Vec<SharedStore>` sequentially, applying the inner node to
/// each item one at a time.
///
/// Use `Batch` when ordering matters or when the inner node is stateful and
/// cannot safely run concurrently. For parallel processing, see [`ParallelBatch`].
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::prelude::*;
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
///
/// #[tokio::main]
/// async fn main() {
///     let summariser = create_node(|store: SharedStore| async move {
///         // summarise store["text"] → store["summary"]
///         store
///     });
///
///     let batch = Batch::new(summariser);
///
///     let inputs: Vec<SharedStore> = (0..3)
///         .map(|_| Arc::new(RwLock::new(std::collections::HashMap::new())))
///         .collect();
///
///     let results: Vec<SharedStore> = batch.call(inputs).await;
/// }
/// ```
#[derive(Clone)]
pub struct Batch<N> {
    node: N,
}

impl<N> Batch<N> {
    /// Wrap `node` in a sequential batch processor.
    pub fn new(node: N) -> Self {
        Self { node }
    }
}

impl<N> Node<Vec<SharedStore>, Vec<SharedStore>> for Batch<N>
where
    N: Node<SharedStore, SharedStore> + Send + Sync + Clone,
{
    fn call(
        &self,
        input: Vec<SharedStore>,
    ) -> Pin<Box<dyn Future<Output = Vec<SharedStore>> + Send + '_>> {
        let node = self.node.clone();
        Box::pin(async move {
            let mut results = Vec::new();
            for store in input {
                let result = node.call(store).await;
                results.push(result);
            }
            results
        })
    }
}

/// Processes a `Vec<SharedStore>` concurrently, applying the inner node to
/// each item in parallel up to `concurrency_limit` at a time.
///
/// Use `ParallelBatch` when items are independent and throughput matters more
/// than ordering. The output order matches the input order.
///
/// Default concurrency limit is **10**. Override with
/// [`with_concurrency_limit`](Self::with_concurrency_limit).
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::prelude::*;
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
///
/// #[tokio::main]
/// async fn main() {
///     let embedder = create_node(|store: SharedStore| async move {
///         // embed store["text"] → store["embedding"]
///         store
///     });
///
///     let batch = ParallelBatch::new(embedder).with_concurrency_limit(5);
///
///     let inputs: Vec<SharedStore> = (0..20)
///         .map(|_| Arc::new(RwLock::new(std::collections::HashMap::new())))
///         .collect();
///
///     let results: Vec<SharedStore> = batch.call(inputs).await;
/// }
/// ```
#[derive(Clone)]
pub struct ParallelBatch<N> {
    node: N,
    concurrency_limit: usize,
}

impl<N> ParallelBatch<N> {
    /// Wrap `node` in a parallel batch processor with the default concurrency
    /// limit of 10.
    pub fn new(node: N) -> Self {
        Self {
            node,
            concurrency_limit: 10,
        }
    }

    /// Override the maximum number of items processed simultaneously.
    pub fn with_concurrency_limit(mut self, limit: usize) -> Self {
        self.concurrency_limit = limit;
        self
    }
}

impl<N> Node<Vec<SharedStore>, Vec<SharedStore>> for ParallelBatch<N>
where
    N: Node<SharedStore, SharedStore> + Send + Sync + Clone,
{
    fn call(
        &self,
        input: Vec<SharedStore>,
    ) -> Pin<Box<dyn Future<Output = Vec<SharedStore>> + Send + '_>> {
        let node = self.node.clone();
        let limit = self.concurrency_limit;
        Box::pin(async move {
            let stream = futures::stream::iter(input.into_iter().map(|store| {
                let node = node.clone();
                async move { node.call(store).await }
            }));
            stream.buffer_unordered(limit).collect().await
        })
    }
}
