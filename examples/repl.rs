use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let mut flow = Flow::new();

    // 1. READ NODE
    let read_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            print!("REPL> ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let input = input.trim().to_string();

            let mut guard = store.lock().await;

            if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
                // Exit the loop by setting an action that has no outgoing edge
                guard.insert("action".to_string(), Value::String("exit".to_string()));
            } else {
                guard.insert("user_input".to_string(), Value::String(input));
                guard.insert("action".to_string(), Value::String("eval".to_string()));
            }
            drop(guard);
            store
        })
    });

    // 2. EVALUATE NODE
    let eval_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let mut guard = store.lock().await;

            if let Some(Value::String(input)) = guard.get("user_input") {
                // Perform evaluation here (e.g., LLM call, math parsing)
                // For this example, we just reverse the string
                let result = input.chars().rev().collect::<String>();

                guard.insert("eval_result".to_string(), Value::String(result));
            }

            guard.insert("action".to_string(), Value::String("print".to_string()));
            drop(guard);
            store
        })
    });

    // 3. PRINT NODE
    let print_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let mut guard = store.lock().await;

            if let Some(Value::String(result)) = guard.get("eval_result") {
                println!("Result: {}\n", result);
            }

            // Loop back to the read step
            guard.insert("action".to_string(), Value::String("read".to_string()));
            drop(guard);
            store
        })
    });

    // Add nodes to the flow
    flow.add_node("read_step", read_node);
    flow.add_node("eval_step", eval_node);
    flow.add_node("print_step", print_node);

    // Define the cyclic transitions based on the "action" key
    // the "action" here in this example are `eval, print, read`.
    flow.add_edge("read_step", "eval", "eval_step");
    flow.add_edge("eval_step", "print", "print_step");
    flow.add_edge("print_step", "read", "read_step");

    // Initialize the store and set the starting node
    let store = Arc::new(Mutex::new(HashMap::new()));

    println!("Starting AgentFlow REPL. Type 'exit' to quit.");

    // Execute the flow. It will run in a loop until "exit" is typed (which has no connected edge)
    let _final_store = flow.run(store).await;

    println!("REPL terminated.");
}
