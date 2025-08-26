use crate::core::node::{Node, SharedStore};
use crate::patterns::workflow::Workflow;

/// BatchFlow: runs a workflow for each batch of parameter sets, like Python's BatchFlow.
pub struct BatchFlow {
    pub workflow: Workflow,
}

impl BatchFlow {
    pub fn new(workflow: Workflow) -> Self {
        Self { workflow }
    }

    /// Run the workflow for each batch of parameter sets.
    pub async fn run(
        &self,
        shared: SharedStore,
        batch_params: Vec<std::collections::HashMap<String, serde_json::Value>>,
    ) -> SharedStore {
        for params in batch_params {
            let mut wf = self.workflow.clone();
            wf.set_params(params);
            // Merge params into shared store
            let mut store = shared.lock().unwrap();
            for (k, v) in wf.params.iter() {
                store.entry(k.clone()).or_insert(v.clone());
            }
            drop(store);
            // Run the workflow
            let _ = wf.call(shared.clone()).await;
        }
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
