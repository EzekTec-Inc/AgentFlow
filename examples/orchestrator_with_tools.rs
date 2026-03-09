/*!
# Example: orchestrator_with_tools.rs

**Purpose:**
Demonstrates a Main Orchestrator Agent that manages a Sub-Agent.
The Sub-Agent operates in a ReAct (Reason + Act) loop, utilizing a
Tool Node to execute real commands before returning the final
result to the Orchestrator.

**How it works:**
1. **Orchestrator**: Receives a complex task, delegates a sub-task to the ReAct Flow.
2. **ReAct Flow (Sub-Agent)**:
    - **Reasoner Node**: Decides if it needs a tool or has the final answer.
    - **Tool Node**: Executes a local shell command (e.g., getting the current date).
3. **Orchestrator**: Receives the final answer from the Sub-Agent and summarizes it.
*/

use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use agentflow::utils::tool::create_tool_node;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    println!("=== Orchestrator with Tool-Using Sub-Agent Example ===\n");

    // ========================================================================
    // 1. Build the Main Orchestrator Node
    // ========================================================================
    let orchestrator_node = create_node(move |store: SharedStore| {
        Box::pin(async move {
            let mut guard = store.write().await;
            let main_task = guard
                .get("main_task")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            println!("[Orchestrator] Received main task: '{}'", main_task);
            println!("[Orchestrator] Delegating sub-task to the Tool-Using Agent...\n");

            // Set up the context for the sub-agent
            guard.insert(
                "sub_task".to_string(),
                Value::String("Find out the current system date and time.".to_string()),
            );
            drop(guard);

            // ----------------------------------------------------------------
            // 2. Build and run the Sub-Agent Flow (ReAct Pattern)
            // ----------------------------------------------------------------
            let mut react_flow = Flow::new();

            // Reasoner Node: Analyzes the task and decides whether to use the tool
            let reasoner_node = create_node(|s: SharedStore| {
                Box::pin(async move {
                    let mut g = s.write().await;

                    let task = g
                        .get("sub_task")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tool_output = g
                        .get("date_tool_stdout")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    println!("  [Sub-Agent] Thinking about task: '{}'", task);

                    if let Some(output) = tool_output {
                        println!("  [Sub-Agent] Received tool output: '{}'", output.trim());
                        // We have the answer now
                        g.insert(
                            "sub_task_result".to_string(),
                            Value::String(format!("The system date is {}.", output.trim())),
                        );
                        // Emit 'done' to break out of the flow
                        g.insert("action".to_string(), Value::String("done".to_string()));
                    } else {
                        println!("  [Sub-Agent] I need the current date. Calling 'date_tool'.");
                        g.insert("action".to_string(), Value::String("use_tool".to_string()));
                    }

                    drop(g);
                    s
                })
            });

            // Tool Node: A real shell command tool using agentflow's built-in tool node
            // Note: `create_tool_node` stores its stdout in `{tool_name}_output` -> `date_tool_output`
            // and emits a "default" action when finished.
            let date_tool_node = create_tool_node("date_tool", "date", vec![]);

            react_flow.add_node("reasoner", reasoner_node);
            react_flow.add_node("tool_executor", date_tool_node);

            // Edges for the ReAct loop
            react_flow.add_edge("reasoner", "use_tool", "tool_executor");
            // Since `create_tool_node` doesn't modify the "action" key, it remains "use_tool".
            // So we route "use_tool" from tool_executor back to reasoner.
            react_flow.add_edge("tool_executor", "use_tool", "reasoner");

            // Run the sub-agent flow with the shared store
            let store = react_flow.run(store).await;

            // ----------------------------------------------------------------
            // 3. Orchestrator finalized
            // ----------------------------------------------------------------
            let mut guard = store.write().await;
            let sub_result = guard
                .get("sub_task_result")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            println!("\n[Orchestrator] Sub-Agent completed its task.");
            println!("[Orchestrator] Final Report Compilation:");
            println!("  Task: {}", main_task);
            println!("  Result: {}", sub_result);

            guard.insert(
                "final_report".to_string(),
                Value::String(format!("Task: {} | Result: {}", main_task, sub_result)),
            );
            drop(guard);

            store
        })
    });

    // ========================================================================
    // 4. Execute the Orchestrator
    // ========================================================================
    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));

    // Set initial context
    {
        let mut guard = store.write().await;
        guard.insert(
            "main_task".to_string(),
            Value::String("Generate a daily system report.".to_string()),
        );
    }

    // Since an Agent wraps a single node, we can run our Orchestrator node directly
    // or wrap it in a Flow. Here we just call it directly as a node.
    let final_store = orchestrator_node.call(store).await;

    let guard = final_store.write().await;
    println!("\n=== Final Store Output ===");
    println!(
        "Final Report: {:?}",
        guard.get("final_report").and_then(|v| v.as_str())
    );
}
