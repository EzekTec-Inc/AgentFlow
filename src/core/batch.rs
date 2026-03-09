use crate::core::node::{Node, SharedStore};
use futures::stream::{StreamExt};
use std::future::Future;
use std::pin::Pin;

/// Batch node processes lists of items sequentially
#[derive(Clone)]
pub struct Batch<N> {
    node: N,
}

impl<N> Batch<N> {
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

/// ParallelBatch processes items concurrently
#[derive(Clone)]
pub struct ParallelBatch<N> {
    node: N,
    concurrency_limit: usize,
}

impl<N> ParallelBatch<N> {
    pub fn new(node: N) -> Self {
        Self { 
            node,
            concurrency_limit: 10, // Default limit
        }
    }

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
