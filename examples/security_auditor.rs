use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use agentflow::core::parallel::ParallelFlow;
use agentflow::utils::tool::create_tool_node;
use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::client::ProviderClient;
use rig::providers::openai::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    println!("Starting Security Auditor Workflow...");

    let store = Arc::new(RwLock::new(HashMap::new()));
    store
        .write()
        .await
        .insert("target_dir".to_string(), Value::String(".".to_string()));

    // 1. Crawler Flow: runs cargo clippy
    let mut crawler = Flow::new();
    crawler.add_node(
        "run_clippy",
        create_tool_node(
            "clippy",
            "cargo",
            vec!["clippy".into(), "--message-format=json".into()],
        ),
    );

    // 2. Parallel Analysis Fan-out
    let mut branch_clippy = Flow::new();
    branch_clippy.add_node(
        "analyze_clippy",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let clippy_out = store
                    .read()
                    .await
                    .get("clippy_stdout")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let openai_client = Client::from_env();
                let agent = openai_client
                    .agent("gpt-4o-mini")
                    .preamble("Analyze the clippy output for potential logic bugs and formatting issues. Keep it concise.")
                    .build();

                let prompt = format!("Clippy output (truncated to first 2000 chars): {:.2000}", clippy_out);
                let resp: String = agent
                    .prompt(&prompt)
                    .await
                    .unwrap_or_else(|e| format!("Error: {}", e));

                store
                    .write()
                    .await
                    .insert("clippy_analysis".to_string(), Value::String(resp));
                store
            })
        }),
    );

    let mut branch_secrets = Flow::new();
    branch_secrets.add_node(
        "scan_secrets",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let openai_client = Client::from_env();
                let agent = openai_client
                    .agent("gpt-4o-mini")
                    .preamble("You are a security auditor.")
                    .build();

                let resp: String = agent
                    .prompt("Provide a brief mock analysis about hardcoded secrets in a typical Rust codebase.")
                    .await
                    .unwrap_or_else(|e| format!("Error: {}", e));

                store
                    .write()
                    .await
                    .insert("secrets_analysis".to_string(), Value::String(resp));
                store
            })
        }),
    );

    let parallel =
        ParallelFlow::new(vec![branch_clippy, branch_secrets]).with_merge(|_initial, results| {
            let merged = Arc::new(RwLock::new(HashMap::new()));
            Box::pin(async move {
                let mut guard = merged.write().await;
                for result in results {
                    for (k, v) in result.read().await.iter() {
                        guard.insert(k.clone(), v.clone());
                    }
                }
                drop(guard);
                merged
            })
        });

    // 3. Synthesis Flow
    let mut synthesis = Flow::new();
    synthesis.add_node(
        "generate_report",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let clippy = store
                    .read()
                    .await
                    .get("clippy_analysis")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let secrets = store
                    .read()
                    .await
                    .get("secrets_analysis")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let openai_client = Client::from_env();
                let agent = openai_client
                    .agent("gpt-4o-mini")
                    .preamble("You write markdown security reports.")
                    .build();

                let resp: String = agent
                    .prompt(&format!(
                        "Compile this into a markdown security report:\n\nClippy:\n{}\n\nSecrets:\n{}",
                        clippy, secrets
                    ))
                    .await
                    .unwrap_or_else(|e| format!("Error: {}", e));

                store
                    .write()
                    .await
                    .insert("report".to_string(), Value::String(resp));
                store
            })
        }),
    );

    // HITL Node
    synthesis.add_node(
        "hitl_review",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let report = store
                    .read()
                    .await
                    .get("report")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                println!(
                    "\n=== DRAFT SECURITY REPORT ===\n{}\n=============================\n",
                    report
                );

                let ans = inquire::Select::new("Approve report?", vec!["Approve", "Reject"])
                    .prompt()
                    .unwrap_or("Reject")
                    .to_string();

                if ans == "Approve" {
                    println!("Report approved! Saving to disk...");
                    // Write to disk would happen here
                } else {
                    println!("Report rejected. Halting.");
                }
                store
            })
        }),
    );

    // Run the pipeline
    println!("Running crawler...");
    let store = crawler.run(store).await;

    println!("Running parallel analysis...");
    let store = parallel.run(store).await;

    println!("Running synthesis & review...");
    let _store = synthesis.run(store).await;

    println!("Security Auditor Workflow completed.");
    Ok(())
}
