use crate::core::batch::Batch;
use crate::core::node::{Node, SharedStore};
use std::future::Future;
use std::pin::Pin;

#[derive(Clone)]
/// MapReduce splits data tasks into Map and Reduce steps
pub struct MapReduce<M, R> {
    pub mapper: Batch<M>,
    pub reducer: R,
}

impl<M, R> MapReduce<M, R> {
    pub fn new(mapper: Batch<M>, reducer: R) -> Self {
        Self { mapper, reducer }
    }

    pub async fn run(&self, inputs: Vec<SharedStore>) -> SharedStore
    where
        M: Node<SharedStore, SharedStore> + Send + Sync + Clone,
        R: Node<Vec<SharedStore>, SharedStore> + Send + Sync,
    {
        let mapped = self.mapper.call(inputs).await;
        self.reducer.call(mapped).await
    }
}

impl<M, R> Node<Vec<SharedStore>, SharedStore> for MapReduce<M, R>
where
    M: Node<SharedStore, SharedStore> + Send + Sync + Clone,
    R: Node<Vec<SharedStore>, SharedStore> + Send + Sync + Clone,
{
    fn call(
        &self,
        input: Vec<SharedStore>,
    ) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
        Box::pin(self.run(input))
    }
}
