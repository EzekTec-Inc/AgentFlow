/*!
# Example: hitl.rs

**Purpose:**
Demonstrates the native Human-in-the-Loop (HITL) pattern, which suspends execution until specific input is provided.

**How it works:**
- Creates a flow with a standard node and a HITL node.
- The HITL node is configured to check for the `human_approval` key.
- The first run suspends because the key is missing.
- After simulating human input by inserting the key into the store, the second run succeeds.

**How to adapt:**
- Use `create_hitl_node` in your flows to pause for external input (e.g., API webhook, user CLI input).
- Catch `AgentFlowError::Suspended` to handle the pause gracefully.

**Example:**
```rust
flow.add_result_node("hitl", create_hitl_node("human_approval", "next_step", "Need approval"));
```
*/

use agentflow::core::error::AgentFlowError;
use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use agentflow::patterns::hitl::create_hitl_node;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    println!("=== HITL Pattern Example ===\n");

    let mut flow = Flow::new();

    // 1. Initial Processing Node
    flow.add_node(
        "step1",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                println!("[step1] Running initial process...");
                let mut guard = store.write().await;
                guard.insert("status".to_string(), json!("step1_complete"));
                drop(guard);
                store
            })
        }),
    );

    // 2. HITL Node
    flow.add_result_node(
        "approval_gate",
        create_hitl_node(
            "human_approval",          // key to look for
            "final_step",              // action to take if present
            "Awaiting human approval", // reason for suspension
        ),
    );

    // 3. Final Processing Node
    flow.add_node(
        "final_step",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                println!("[final_step] Running final process after approval...");
                let mut guard = store.write().await;
                guard.insert("status".to_string(), json!("workflow_complete"));
                drop(guard);
                store
            })
        }),
    );

    flow.add_edge("step1", "default", "approval_gate");
    flow.add_edge("approval_gate", "final_step", "final_step");

    flow.set_start("step1");

    let store = Arc::new(RwLock::new(HashMap::new()));

    // Run 1: Should suspend because "human_approval" is missing.
    println!("--- Run 1 ---");
    let result1 = flow.run_safe(store.clone()).await;
    match result1 {
        Err(AgentFlowError::Suspended(reason)) => {
            println!("Flow suspended correctly: {}", reason);
        }
        _ => println!("Unexpected result!"),
    }

    // Simulate Human Interaction: Provide the required input
    println!("\n[Human] Providing approval...");
    {
        let mut guard = store.write().await;
        guard.insert("human_approval".to_string(), json!(true));
    }

    // Run 2: Resume the flow
    println!("\n--- Run 2 ---");
    flow.set_start("approval_gate");
    let result2 = flow.run_safe(store.clone()).await;
    match result2 {
        Ok(final_store) => {
            let guard = final_store.read().await;
            println!(
                "Flow completed successfully! Final status: {:?}",
                guard.get("status")
            );
        }
        Err(e) => println!("Flow failed: {:?}", e),
    }
}
