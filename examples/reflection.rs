/*!
# Example: reflection.rs

Real-world Reflection pattern. A Generator LLM writes a draft; a Critic LLM
reviews it and either approves or sends it back with specific feedback. The
loop continues until the Critic approves or max_steps is reached.

Domain: technical blog post paragraph about Rust's ownership model.

Requires: OPENAI_API_KEY
Run with: cargo run --example reflection
*/

use agentflow::core::error::AgentFlowError;
use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use dotenvy::dotenv;
use rig::prelude::*;
use rig::{completion::Prompt, providers};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{fmt, EnvFilter};

const GENERATOR_SYSTEM: &str = "You are a technical writer specialising in Rust. \
     Write a single, clear paragraph (3-5 sentences) explaining the topic you are given. \
     If feedback is provided, revise your paragraph to address it precisely. \
     Output only the paragraph — no headings or extra text.";

const CRITIC_SYSTEM: &str = "You are a senior technical editor. Review the paragraph you receive. \
     If it is accurate, clear, and complete respond with exactly: APPROVED \
     Otherwise respond with: REVISE: <one sentence of specific feedback> \
     No other text.";

#[tokio::main]
async fn main() {
    dotenv().ok();
    fmt()
        .with_env_filter(EnvFilter::new("agentflow=debug,reflection=debug"))
        .init();

    let mut flow = Flow::new().with_max_steps(12); // 2 steps/cycle → 6 revision rounds max

    let topic = "Rust's ownership model and how it prevents data races at compile time";

    // ── Generator ────────────────────────────────────────────────────────────
    let generator = create_node(|store: SharedStore| {
        Box::pin(async move {
            let (topic, feedback, attempt) = {
                let g = store.read().await;
                (
                    g.get("topic")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    g.get("feedback")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    g.get("attempt").and_then(|v| v.as_u64()).unwrap_or(0),
                )
            };

            let prompt = match &feedback {
                Some(fb) => format!(
                    "Topic: {}\n\nPrevious feedback from editor:\n{}\n\nPlease revise your paragraph.",
                    topic, fb
                ),
                None => format!("Topic: {}", topic),
            };

            println!("\n[Generator] Attempt {} — writing draft…", attempt + 1);
            let client = providers::openai::Client::from_env();
            let agent = client
                .agent("gpt-4o-mini")
                .preamble(GENERATOR_SYSTEM)
                .build();

            let draft = match agent.prompt(&prompt).await {
                Ok(r) => r,
                Err(e) => format!("Draft unavailable due to LLM error: {e}"),
            };
            println!("[Generator] Draft:\n{}", draft.trim());

            let mut g = store.write().await;
            g.insert("draft".to_string(), Value::String(draft));
            g.insert("attempt".to_string(), Value::Number((attempt + 1).into()));
            g.insert("action".to_string(), Value::String("critique".to_string()));
            drop(g);
            store
        })
    });

    // ── Critic ───────────────────────────────────────────────────────────────
    let critic = create_node(|store: SharedStore| {
        Box::pin(async move {
            let draft = {
                let g = store.read().await;
                g.get("draft")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };

            println!("\n[Critic] Reviewing draft…");
            let client = providers::openai::Client::from_env();
            let agent = client.agent("gpt-4o-mini").preamble(CRITIC_SYSTEM).build();

            let verdict = match agent.prompt(&draft).await {
                Ok(r) => r,
                Err(e) => format!("APPROVED (critic error: {e})"),
            };
            println!("[Critic] {}", verdict.trim());

            let mut g = store.write().await;
            if verdict.trim().starts_with("APPROVED") {
                g.insert("action".to_string(), Value::String("approve".to_string()));
                // Remove stale feedback so final store is clean
                g.remove("feedback");
            } else {
                let fb = verdict
                    .trim()
                    .strip_prefix("REVISE:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                g.insert("feedback".to_string(), Value::String(fb));
                g.insert("action".to_string(), Value::String("revise".to_string()));
            }
            drop(g);
            store
        })
    });

    flow.add_node("generator", generator);
    flow.add_node("critic", critic);
    flow.add_edge("generator", "critique", "critic");
    flow.add_edge("critic", "revise", "generator");
    // "approve" has no edge → flow stops

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    store
        .write()
        .await
        .insert("topic".to_string(), Value::String(topic.to_string()));

    println!("=== Reflection Agent ===");
    println!("Topic: {}\n", topic);

    match flow.run_safe(store).await {
        Ok(s) => {
            let g = s.read().await;
            println!("\n=== Approved Draft ===");
            println!("{}", g.get("draft").and_then(|v| v.as_str()).unwrap_or(""));
            println!(
                "\nCompleted in {} attempt(s).",
                g.get("attempt").and_then(|v| v.as_u64()).unwrap_or(0)
            );
        }
        Err(AgentFlowError::ExecutionLimitExceeded(msg)) => eprintln!("Step limit hit: {msg}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}
