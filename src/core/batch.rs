use crate::core::node::{Node, SharedStore};
use futures::future::join_all;
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
}

impl<N> ParallelBatch<N> {
    pub fn new(node: N) -> Self {
        Self { node }
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
        Box::pin(async move {
            let futures = input.into_iter().map(|store| {
                let node = node.clone();
                async move { node.call(store).await }
            });

            join_all(futures).await
        })
    }
}
