/*!
# Example: routing.rs

Real-world LLM-powered intent routing. A Triage node calls an LLM to classify
a customer message into one of three intents (tech_support, billing, general)
and routes it to the appropriate specialist agent — also LLM-backed.

Domain: customer service inbox routing.

Requires: OPENAI_API_KEY
Run with: cargo run --example routing
*/

use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use dotenvy::dotenv;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{fmt, EnvFilter};
use rig::prelude::*;
use rig::{completion::Prompt, providers};

const TRIAGE_SYSTEM: &str =
    "You are a customer-service triage bot. Classify the message into exactly one category: \
     tech_support, billing, or general. \
     Respond with only the category name — no other text.";

const TECH_SYSTEM: &str =
    "You are a technical support specialist. Respond to the customer's issue concisely \
     (2-3 sentences). Be empathetic and actionable.";

const BILLING_SYSTEM: &str =
    "You are a billing specialist. Respond to the customer's billing query concisely \
     (2-3 sentences). Be clear about next steps.";

const GENERAL_SYSTEM: &str =
    "You are a helpful customer service agent. Respond to the customer's message \
     concisely (2-3 sentences). Be friendly and helpful.";

async fn llm_reply(system: &str, message: &str) -> String {
    let client = providers::openai::Client::from_env();
    let agent  = client.agent("gpt-4o-mini").preamble(system).build();
    match agent.prompt(message).await {
        Ok(r)  => r,
        Err(e) => format!("Service temporarily unavailable: {e}"),
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    fmt().with_env_filter(EnvFilter::new("agentflow=debug,routing=debug")).init();

    let mut flow = Flow::new();

    // ── Triage node: LLM classifies intent ───────────────────────────────────
    let triage = create_node(|store: SharedStore| {
        Box::pin(async move {
            let message = {
                let g = store.read().await;
                g.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string()
            };

            println!("[Triage] Classifying: \"{}\"", message);
            let client = providers::openai::Client::from_env();
            let agent  = client.agent("gpt-4o-mini").preamble(TRIAGE_SYSTEM).build();

            let intent = match agent.prompt(&message).await {
                Ok(r)  => r.trim().to_lowercase(),
                Err(e) => { eprintln!("[Triage] LLM error: {e}"); "general".to_string() }
            };

            // Normalise to known intents
            let intent = if intent.contains("tech") { "tech_support" }
                         else if intent.contains("bill") { "billing" }
                         else { "general" };

            println!("[Triage] Intent: {}", intent);
            store.write().await.insert("action".to_string(), Value::String(intent.to_string()));
            store
        })
    });

    // ── Specialist nodes ─────────────────────────────────────────────────────
    let tech_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let msg = store.read().await.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
            println!("[TechSupport] Handling request…");
            let reply = llm_reply(TECH_SYSTEM, &msg).await;
            println!("[TechSupport] {}", reply.trim());
            store.write().await.insert("response".to_string(), Value::String(reply));
            store
        })
    });

    let billing_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let msg = store.read().await.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
            println!("[Billing] Handling request…");
            let reply = llm_reply(BILLING_SYSTEM, &msg).await;
            println!("[Billing] {}", reply.trim());
            store.write().await.insert("response".to_string(), Value::String(reply));
            store
        })
    });

    let general_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let msg = store.read().await.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
            println!("[General] Handling request…");
            let reply = llm_reply(GENERAL_SYSTEM, &msg).await;
            println!("[General] {}", reply.trim());
            store.write().await.insert("response".to_string(), Value::String(reply));
            store
        })
    });

    flow.add_node("triage",       triage);
    flow.add_node("tech_support", tech_node);
    flow.add_node("billing",      billing_node);
    flow.add_node("general",      general_node);

    flow.add_edge("triage", "tech_support", "tech_support");
    flow.add_edge("triage", "billing",      "billing");
    flow.add_edge("triage", "general",      "general");

    let messages = [
        "My application keeps crashing with a segmentation fault after the latest update.",
        "I was charged twice for my subscription this month — please help.",
        "Hi, I just wanted to say your product is fantastic. Keep it up!",
    ];

    println!("=== LLM-Powered Customer Service Router ===\n");

    for (i, msg) in messages.iter().enumerate() {
        println!("─── Message {} ─────────────────────────────────", i + 1);
        println!("Customer: {}\n", msg);

        let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
        store.write().await.insert("message".to_string(), Value::String(msg.to_string()));

        let result = flow.run(store).await;
        let g = result.read().await;
        println!("\nAgent reply: {}\n", g.get("response").and_then(|v| v.as_str()).unwrap_or("(no response)"));
    }
}
