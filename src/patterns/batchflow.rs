use crate::core::node::{Node, SharedStore};
use crate::patterns::workflow::Workflow;
use std::time::Instant;
use tracing::{debug, info, instrument};

/// BatchFlow: runs a workflow for each batch of parameter sets, like Python's BatchFlow.
pub struct BatchFlow {
    pub workflow: Workflow,
}

impl BatchFlow {
    pub fn new(workflow: Workflow) -> Self {
        Self { workflow }
    }

    /// Run the workflow for each batch of parameter sets.
    #[instrument(name = "batchflow.run", skip(self, shared, batch_params), fields(batch_count = batch_params.len()))]
    pub async fn run(
        &self,
        shared: SharedStore,
        batch_params: Vec<std::collections::HashMap<String, serde_json::Value>>,
    ) -> SharedStore {
        let t = Instant::now();
        let total = batch_params.len();
        debug!(batch_count = total, "BatchFlow: starting");
        for (i, params) in batch_params.into_iter().enumerate() {
            debug!(batch_index = i, "BatchFlow: running batch item");
            let mut wf = self.workflow.clone();
            wf.set_params(params);
            {
                let mut store = shared.write().await;
                for (k, v) in wf.params.iter() {
                    store.insert(k.clone(), v.clone());
                }
            }
            let _ = wf.call(shared.clone()).await;
        }
        info!(
            batch_count = total,
            elapsed_ms = t.elapsed().as_millis(),
            "BatchFlow: complete"
        );
        shared
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
