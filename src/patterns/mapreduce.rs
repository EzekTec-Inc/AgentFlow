use crate::core::node::{Node, SharedStore};
use std::future::Future;
use std::pin::Pin;
use std::time::Instant;
use tracing::{debug, info, instrument};

/// Parallel map then sequential reduce over a collection of [`SharedStore`]s.
///
/// `MapReduce` is ideal for processing large document sets:
///
/// 1. **Map phase** — the `mapper` node is applied to every item in the input
///    batch sequentially via [`Batch`]. Each item is an independent
///    [`SharedStore`].
/// 2. **Reduce phase** — the `reducer` node receives the full
///    `Vec<SharedStore>` of mapped results and aggregates them into a single
///    [`SharedStore`].
///
/// For parallel mapping, wrap your mapper in a [`ParallelBatch`] and supply it
/// to `MapReduce::new` — the constructor accepts any `M: Node<SharedStore, SharedStore>`.
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
///     let mapper = create_node(|store: SharedStore| async move {
///         // summarise store["text"] → store["summary"]
///         store
///     });
///
///     let reducer = create_batch_node(|stores: Vec<SharedStore>| async move {
///         // merge all store["summary"] into a single store
///         let out: SharedStore = Arc::new(RwLock::new(HashMap::new()));
///         out.write().await.insert("total".into(), serde_json::json!(stores.len()));
///         out
///     });
///
///     let mr = MapReduce::new(Batch::new(mapper), reducer);
///
///     let inputs: Vec<SharedStore> = (0..5)
///         .map(|_| Arc::new(RwLock::new(HashMap::new())))
///         .collect();
///
///     let result = mr.run(inputs).await;
/// }
/// ```
///
/// [`ParallelBatch`]: crate::core::batch::ParallelBatch
/// [`Batch`]: crate::core::batch::Batch
#[derive(Clone)]
pub struct MapReduce<M, R> {
    /// The mapper applied to the input batch (e.g., `Batch` or `ParallelBatch`).
    pub mapper: M,
    /// The reducer applied to all mapped results.
    pub reducer: R,
}

impl<M, R> MapReduce<M, R> {
    /// Create a `MapReduce` from a batch mapper and a reducer.
    pub fn new(mapper: M, reducer: R) -> Self {
        Self { mapper, reducer }
    }

    /// Execute the map phase then the reduce phase.
    #[instrument(name = "mapreduce.run", skip(self, inputs), fields(input_count = inputs.len()))]
    pub async fn run(&self, inputs: Vec<SharedStore>) -> SharedStore
    where
        M: Node<Vec<SharedStore>, Vec<SharedStore>> + Send + Sync,
        R: Node<Vec<SharedStore>, SharedStore> + Send + Sync,
    {
        let t = Instant::now();
        debug!(input_count = inputs.len(), "MapReduce: starting map phase");
        let mapped = self.mapper.call(inputs).await;
        debug!(
            mapped_count = mapped.len(),
            "MapReduce: map phase done, starting reduce"
        );
        let result = self.reducer.call(mapped).await;
        info!(elapsed_ms = t.elapsed().as_millis(), "MapReduce: complete");
        result
    }
}

impl<M, R> Node<Vec<SharedStore>, SharedStore> for MapReduce<M, R>
where
    M: Node<Vec<SharedStore>, Vec<SharedStore>> + Send + Sync + Clone,
    R: Node<Vec<SharedStore>, SharedStore> + Send + Sync + Clone,
{
    fn call(
        &self,
        input: Vec<SharedStore>,
    ) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
        Box::pin(self.run(input))
    }
}
