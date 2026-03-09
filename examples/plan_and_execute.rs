use agentflow::core::error::AgentFlowError;
use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() {
    fmt()
        .with_env_filter(EnvFilter::new("agentflow=debug,plan_and_execute=debug"))
        .init();

    // The self-loop on "executor" runs once per task; 3 tasks + 1 planner = 4 steps max.
    // In a real scenario set this to a safe upper bound for your plan size.
    let mut flow = Flow::new().with_max_steps(20);

    // 1. Planner Node: Breaks a complex task into steps.
    let planner_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let mut guard = store.write().await;

            // In a real scenario, this would call an LLM to generate the plan.
            let plan = vec![
                Value::String("Step 1: Research topic".to_string()),
                Value::String("Step 2: Draft outline".to_string()),
                Value::String("Step 3: Write content".to_string()),
            ];

            println!("Planner: Created plan with {} steps.", plan.len());
            guard.insert("plan".to_string(), Value::Array(plan.clone()));
            guard.insert("action".to_string(), Value::String("execute".to_string()));
            drop(guard);
            store
        })
    });

    // 2. Executor Node: Pops the next task and executes it.
    let executor_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let mut guard = store.write().await;

            let mut next_action = "done".to_string();

            if let Some(Value::Array(plan)) = guard.get_mut("plan") {
                if !plan.is_empty() {
                    let task = plan.remove(0);
                    println!(
                        "Executor: Executing task: {}",
                        task.as_str().unwrap_or("")
                    );
                    // In a real scenario, perform the task here.

                    if !plan.is_empty() {
                        next_action = "execute".to_string(); // loop back to executor
                    }
                }
            }

            guard.insert("action".to_string(), Value::String(next_action));
            drop(guard);
            store
        })
    });

    flow.add_node("planner", planner_node);
    flow.add_node("executor", executor_node);

    flow.add_edge("planner", "execute", "executor");
    flow.add_edge("executor", "execute", "executor"); // self-loop for remaining tasks

    let store = Arc::new(RwLock::new(HashMap::new()));

    println!("Starting Plan and Execute Pattern...");
    match flow.run_safe(store).await {
        Ok(_) => println!("Plan and Execute completed."),
        Err(AgentFlowError::ExecutionLimitExceeded(msg)) => {
            eprintln!("Plan and Execute aborted — step limit exceeded: {}", msg);
        }
        Err(e) => eprintln!("Plan and Execute failed: {}", e),
    }
}
