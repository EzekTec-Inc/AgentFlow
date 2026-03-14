/*!
# Example: react.rs

Real-world ReAct (Reason + Act) agent. The LLM decides each turn whether
to call a tool or emit a final answer. Tool execution is a real shell command
(curl-based web fetch is simulated here — swap for any HTTP call).

Requires: OPENAI_API_KEY
Run with: cargo run --example react
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

const SYSTEM: &str = r#"You are a reasoning agent. You have one tool:
  search(query) — returns a web-search snippet.

Respond with EXACTLY one of:
  ACTION: search | QUERY: <search query>
  ANSWER: <final answer>

No other text."#;

fn search(query: &str) -> String {
    // Replace this body with a real HTTP call (Brave, Tavily, SerpAPI, etc.)
    format!(
        "Search snippet for '{}': Vienna is the capital of Austria. \
         City population ~1.9 M, metro ~2.9 M. Major EU cultural hub.",
        query
    )
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    fmt()
        .with_env_filter(EnvFilter::new("agentflow=debug,react=debug"))
        .init();

    let mut flow = Flow::new().with_max_steps(20); // 2 steps/cycle → 10 tool calls max

    // ── Reasoner: calls the LLM ──────────────────────────────────────────────
    let reasoner = create_node(|store: SharedStore| {
        Box::pin(async move {
            let (question, tool_out) = {
                let g = store.read().await;
                (
                    g.get("question")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    g.get("tool_output")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                )
            };

            let user_msg = match &tool_out {
                Some(out) => format!("Question: {}\n\nTool output:\n{}", question, out),
                None => format!("Question: {}", question),
            };

            println!("\n[Reasoner] Calling LLM...");
            let client = providers::openai::Client::from_env();
            let agent = client.agent("gpt-4o-mini").preamble(SYSTEM).build();

            let reply = match agent.prompt(&user_msg).await {
                Ok(r) => r,
                Err(e) => format!("ANSWER: LLM error — {e}"),
            };
            println!("[Reasoner] {}", reply.trim());

            let mut g = store.write().await;
            if reply.trim().starts_with("ANSWER:") {
                let ans = reply
                    .trim()
                    .strip_prefix("ANSWER:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                g.insert("final_answer".to_string(), Value::String(ans));
                g.insert("action".to_string(), Value::String("done".to_string()));
            } else {
                let q = reply
                    .split("QUERY:")
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                g.insert("tool_query".to_string(), Value::String(q));
                g.insert("action".to_string(), Value::String("use_tool".to_string()));
            }
            drop(g);
            store
        })
    });

    // ── Tool executor ────────────────────────────────────────────────────────
    let tool_exec = create_node(|store: SharedStore| {
        Box::pin(async move {
            let query = {
                let g = store.read().await;
                g.get("tool_query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            println!("[Tool] search(\"{}\") …", query);
            let result = search(&query);
            println!("[Tool] {}", result);

            let mut g = store.write().await;
            g.insert("tool_output".to_string(), Value::String(result));
            g.insert("action".to_string(), Value::String("reason".to_string()));
            drop(g);
            store
        })
    });

    flow.add_node("reasoner", reasoner);
    flow.add_node("tool_executor", tool_exec);
    flow.add_edge("reasoner", "use_tool", "tool_executor");
    flow.add_edge("tool_executor", "reason", "reasoner");
    // "done" has no edge → flow stops naturally

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    store.write().await.insert(
        "question".to_string(),
        Value::String("What is the capital of Austria and its approximate population?".to_string()),
    );

    println!("=== ReAct Agent ===");
    println!("Question: What is the capital of Austria and its approximate population?\n");

    match flow.run_safe(store).await {
        Ok(s) => {
            if let Some(ans) = s.read().await.get("final_answer").and_then(|v| v.as_str()) {
                println!("\n[Final Answer] {}", ans);
            }
        }
        Err(AgentFlowError::ExecutionLimitExceeded(msg)) => eprintln!("Step limit hit: {msg}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}
