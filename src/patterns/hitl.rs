use crate::core::error::AgentFlowError;
use crate::core::node::{create_result_node, ResultNode, SharedStore};

/// Creates a Human-in-the-Loop (HITL) node that pauses execution
/// and waits for external input before proceeding.
///
/// The node checks the `SharedStore` for the presence of an `input_key`.
/// If the key exists, it sets the `action` to `continue_action`,
/// and returns `Ok(store)`.
/// If the key does NOT exist, it returns `Err(AgentFlowError::Suspended(reason))`.
pub fn create_hitl_node(
    input_key: &'static str,
    continue_action: &'static str,
    reason: &'static str,
) -> ResultNode {
    create_result_node(move |store: SharedStore| {
        async move {
            let mut guard = store.write().await;
            if guard.contains_key(input_key) {
                // Input found: proceed by routing
                guard.insert(
                    "action".to_string(),
                    serde_json::Value::String(continue_action.to_string()),
                );
                drop(guard);
                Ok(store)
            } else {
                // Input missing: suspend execution.
                drop(guard);
                Err(AgentFlowError::Suspended(reason.to_string()))
            }
        }
    })
}
