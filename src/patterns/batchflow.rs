use crate::core::node::{Node, SharedStore};
use crate::patterns::workflow::Workflow;
use std::time::Instant;
use tracing::{debug, info, instrument};

/// BatchFlow: runs a workflow for each batch of parameter sets, like Python's BatchFlow.
pub struct BatchFlow {
    /// The workflow to execute for each batch item.
    pub workflow: Workflow,
}

impl BatchFlow {
    /// Create a new `BatchFlow` with the given workflow.
    pub fn new(workflow: Workflow) -> Self {
        Self { workflow }
    }

    /// Run the workflow for each batch of parameter sets.
    #[instrument(name = "batchflow.run", skip(self, shared, batch_params), fields(batch_count = batch_params.len()))]
    pub async fn run(
        &self,
        shared: SharedStore,
        batch_params: Vec<std::collections::HashMap<String, serde_json::Value>>,
    ) -> Vec<SharedStore> {
        let t = Instant::now();
        let total = batch_params.len();
        debug!(batch_count = total, "BatchFlow: starting");
        let mut results = Vec::with_capacity(total);
        for (i, params) in batch_params.into_iter().enumerate() {
            debug!(batch_index = i, "BatchFlow: running batch item");
            let mut wf = self.workflow.clone();
            wf.set_params(params);
            
            // Snapshot the shared store for this item
            let item_store = {
                let guard = shared.read().await;
                std::sync::Arc::new(tokio::sync::RwLock::new(guard.clone()))
            };
            
            {
                let mut store = item_store.write().await;
                for (k, v) in wf.params.iter() {
                    store.insert(k.clone(), v.clone());
                }
            }
            let res = wf.call(item_store).await;
            results.push(res);
        }
        info!(
            batch_count = total,
            elapsed_ms = t.elapsed().as_millis(),
            "BatchFlow: complete"
        );
        results
    }
}

// Implement Clone for BatchFlow (requires Workflow: Clone)
impl Clone for BatchFlow {
    fn clone(&self) -> Self {
        BatchFlow {
            workflow: self.workflow.clone(),
        }
    }
}
