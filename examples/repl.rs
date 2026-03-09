/*!
# Example: repl.rs

Real-world LLM-powered REPL. The user types a message; an LLM answers;
the conversation history is kept in the store so the LLM has full context.

Type `exit` or `quit` to stop.

Requires: OPENAI_API_KEY
Run with: cargo run --example repl
*/

use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use dotenvy::dotenv;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::RwLock;
use rig::prelude::*;
use rig::{completion::Prompt, providers};

const SYSTEM: &str =
    "You are a knowledgeable, concise assistant embedded in a terminal REPL. \
     Answer clearly in 1-3 sentences unless asked for more detail. \
     You have access to the conversation history provided in the user message.";

#[tokio::main]
async fn main() {
    dotenv().ok();

    let mut flow = Flow::new();

    // ── Read: get user input from stdin ──────────────────────────────────────
    let read_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            print!("\nYou> ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let input = input.trim().to_string();

            let mut g = store.write().await;
            if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
                g.insert("action".to_string(), Value::String("exit".to_string()));
            } else {
                g.insert("user_input".to_string(), Value::String(input));
                g.insert("action".to_string(),     Value::String("eval".to_string()));
            }
            drop(g);
            store
        })
    });

    // ── Eval: call LLM with full conversation history ────────────────────────
    let eval_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let (user_input, history) = {
                let g = store.read().await;
                let input = g.get("user_input").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let hist  = g.get("history")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                         .filter_map(|v| v.as_str())
                         .collect::<Vec<_>>()
                         .join("\n")
                    })
                    .unwrap_or_default();
                (input, hist)
            };

            let prompt = if history.is_empty() {
                user_input.clone()
            } else {
                format!("Conversation so far:\n{}\n\nUser: {}", history, user_input)
            };

            let client = providers::openai::Client::from_env();
            let agent  = client.agent("gpt-4o-mini").preamble(SYSTEM).build();

            let reply = match agent.prompt(&prompt).await {
                Ok(r)  => r,
                Err(e) => format!("(LLM error: {e})"),
            };

            let mut g = store.write().await;

            // Append to history
            let new_entries = vec![
                Value::String(format!("User: {}", user_input)),
                Value::String(format!("Assistant: {}", reply.trim())),
            ];
            match g.get_mut("history") {
                Some(Value::Array(hist)) => hist.extend(new_entries),
                _ => { g.insert("history".to_string(), Value::Array(new_entries)); }
            }

            g.insert("llm_reply".to_string(), Value::String(reply));
            g.insert("action".to_string(),    Value::String("print".to_string()));
            drop(g);
            store
        })
    });

    // ── Print: show LLM reply ────────────────────────────────────────────────
    let print_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let reply = store.read().await
                .get("llm_reply")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            println!("\nAssistant> {}", reply.trim());

            store.write().await.insert("action".to_string(), Value::String("read".to_string()));
            store
        })
    });

    flow.add_node("read",  read_node);
    flow.add_node("eval",  eval_node);
    flow.add_node("print", print_node);

    flow.add_edge("read",  "eval",  "eval");
    flow.add_edge("eval",  "print", "print");
    flow.add_edge("print", "read",  "read");
    // "exit" has no edge → flow stops naturally

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));

    println!("=== AgentFlow LLM REPL ===");
    println!("Type your message and press Enter. Type 'exit' to quit.\n");

    flow.run(store).await;
    println!("\nGoodbye.");
}
