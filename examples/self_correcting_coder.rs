use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, Node, SharedStore};
use agentflow::patterns::agent::Agent;
use rig::client::CompletionClient;
use rig::client::ProviderClient;
use rig::completion::Prompt;
use rig::providers::openai::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    println!("Starting Self-Correcting Coder Workflow...");

    let store = Arc::new(RwLock::new(HashMap::new()));
    store.write().await.insert(
        "task".to_string(),
        Value::String(
            "Write a Rust function that calculates the nth Fibonacci number. Wrap it in fn main() { println!(\"{}\", fib(10)); }"
                .to_string(),
        ),
    );

    // Sub-flow that generates code and compiles it
    let mut generate_and_test = Flow::new();

    // Node 1: Generator
    generate_and_test.add_node(
        "generate_code",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let task = store
                    .read()
                    .await
                    .get("task")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let previous_error = store
                    .read()
                    .await
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // Clear previous error before generation
                store.write().await.remove("error");

                let prompt = if previous_error.is_empty() {
                    format!("Task: {}", task)
                } else {
                    format!(
                        "Task: {}\n\nYour previous code failed to compile with this error:\n{}\n\nFix the code.",
                        task, previous_error
                    )
                };

                println!("Generating code...");
                let openai_client = Client::from_env();
                let agent = openai_client
                    .agent("gpt-4o-mini")
                    .preamble("You write raw Rust code. Do not use markdown blocks (```rust), just output the raw code. Do not add explanations.")
                    .build();

                let code: String = agent
                    .prompt(&prompt)
                    .await
                    .unwrap_or_else(|e| format!("// Error: {}", e));

                // Remove markdown formatting if the LLM adds it anyway
                let code = code
                    .replace("```rust\n", "")
                    .replace("```rust", "")
                    .replace("```\n", "")
                    .replace("```", "");

                store
                    .write()
                    .await
                    .insert("code".to_string(), Value::String(code));
                store
            })
        }),
    );

    // Node 2: Evaluator
    generate_and_test.add_node(
        "evaluate_code",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let code = store
                    .read()
                    .await
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let temp_dir = env::temp_dir();
                let file_path = temp_dir.join("agentflow_coder_test.rs");
                fs::write(&file_path, code).unwrap();

                println!("Compiling code...");
                // Run rustc
                let output = Command::new("rustc")
                    .arg(&file_path)
                    .arg("--out-dir")
                    .arg(&temp_dir)
                    .output()
                    .unwrap();

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    println!("Compilation failed. Storing error for retry...");
                    store
                        .write()
                        .await
                        .insert("error".to_string(), Value::String(stderr));
                } else {
                    println!("Compilation succeeded!");
                }

                store
            })
        }),
    );

    // Wrap the sub-flow in an Agent with max_retries = 3.
    // AgentFlow automatically retries the inner node (the Flow) if the output store contains the "error" key!
    let agent = Agent::with_retry(generate_and_test, 3, 1000);

    println!("Running the Self-Correcting Coder Agent (up to 3 retries)...");
    let result = agent.call(store).await;

    let final_code = result
        .read()
        .await
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let final_error = result
        .read()
        .await
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if final_error.is_empty() {
        println!("\n=== Final Successful Code ===\n{}", final_code);
    } else {
        println!("\n=== Failed After Max Retries ===\n{}", final_error);
    }

    Ok(())
}
