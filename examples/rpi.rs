/*!
# Example: rpi.rs

Real-world Research → Plan → Implement → Verify (RPI) workflow with live LLM
calls at every phase. Each phase passes its output to the next via the shared
store. The Verify phase returns either PASS or FAIL:<reason>; on FAIL the flow
loops back to Implement for a revision.

Domain: generating a production-quality Rust function from a spec.

Requires: OPENAI_API_KEY
Run with: cargo run --example rpi
*/

use agentflow::core::node::{create_node, SharedStore};
use agentflow::patterns::rpi::RpiWorkflow;
use dotenvy::dotenv;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{fmt, EnvFilter};
use rig::prelude::*;
use rig::{completion::Prompt, providers};

async fn llm(system: &str, user: &str) -> String {
    let client = providers::openai::Client::from_env();
    let agent  = client.agent("gpt-4o-mini").preamble(system).build();
    match agent.prompt(user).await {
        Ok(r)  => r,
        Err(e) => format!("LLM error: {e}"),
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    fmt().with_env_filter(EnvFilter::new("agentflow=debug,rpi=debug")).init();

    let goal = "Write a Rust function `fn word_count(s: &str) -> HashMap<String, usize>` \
                that counts word frequencies in a string (case-insensitive). \
                Include doc-comment and a unit test.";

    // ── Research ─────────────────────────────────────────────────────────────
    let research = create_node(|store: SharedStore| {
        Box::pin(async move {
            let goal = store.read().await.get("goal").and_then(|v| v.as_str()).unwrap_or("").to_string();
            println!("[Research] Gathering context for goal…");

            let context = llm(
                "You are a Rust expert. Given a coding goal, list the key Rust concepts, \
                 stdlib types, and edge cases that are relevant. Be concise.",
                &goal,
            ).await;
            println!("[Research] Context:\n{}\n", context.trim());

            let mut g = store.write().await;
            g.insert("context".to_string(), Value::String(context));
            g.insert("action".to_string(),  Value::String("default".to_string()));
            drop(g);
            store
        })
    });

    // ── Plan ─────────────────────────────────────────────────────────────────
    let plan = create_node(|store: SharedStore| {
        Box::pin(async move {
            let (goal, context) = {
                let g = store.read().await;
                (
                    g.get("goal").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    g.get("context").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                )
            };
            println!("[Plan] Creating implementation plan…");

            let plan_text = llm(
                "You are a Rust software architect. Given a goal and research context, \
                 produce a concise numbered implementation plan (steps only, no code).",
                &format!("Goal: {}\n\nContext:\n{}", goal, context),
            ).await;
            println!("[Plan] Plan:\n{}\n", plan_text.trim());

            let mut g = store.write().await;
            g.insert("plan".to_string(),   Value::String(plan_text));
            g.insert("action".to_string(), Value::String("default".to_string()));
            drop(g);
            store
        })
    });

    // ── Implement ─────────────────────────────────────────────────────────────
    let implement = create_node(|store: SharedStore| {
        Box::pin(async move {
            let (goal, plan_text, feedback) = {
                let g = store.read().await;
                (
                    g.get("goal").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    g.get("plan").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    g.get("verify_feedback").and_then(|v| v.as_str()).map(|s| s.to_string()),
                )
            };

            let prompt = match &feedback {
                Some(fb) => format!(
                    "Goal: {}\nPlan:\n{}\n\nPrevious implementation was rejected.\nFeedback: {}\n\nFix the code.",
                    goal, plan_text, fb
                ),
                None => format!("Goal: {}\nPlan:\n{}\n\nWrite the Rust code now.", goal, plan_text),
            };

            println!("[Implement] Writing code…");
            let code = llm(
                "You are a Rust developer. Output only the Rust code — no explanation, no markdown fences.",
                &prompt,
            ).await;
            println!("[Implement] Code:\n{}\n", code.trim());

            let mut g = store.write().await;
            g.insert("code".to_string(),   Value::String(code));
            g.insert("action".to_string(), Value::String("default".to_string()));
            drop(g);
            store
        })
    });

    // ── Verify ───────────────────────────────────────────────────────────────
    let verify = create_node(|store: SharedStore| {
        Box::pin(async move {
            let (goal, code) = {
                let g = store.read().await;
                (
                    g.get("goal").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    g.get("code").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                )
            };

            println!("[Verify] Reviewing implementation…");
            let verdict = llm(
                "You are a strict Rust code reviewer. Given a goal and implementation, \
                 respond with exactly:\n  PASS\n  or\n  FAIL: <one-sentence reason>\nNo other text.",
                &format!("Goal: {}\n\nCode:\n{}", goal, code),
            ).await;
            println!("[Verify] Verdict: {}", verdict.trim());

            let mut g = store.write().await;
            if verdict.trim().starts_with("PASS") {
                g.remove("verify_feedback");
                g.insert("action".to_string(), Value::String("done".to_string()));
            } else {
                let reason = verdict.trim().strip_prefix("FAIL:").unwrap_or("").trim().to_string();
                g.insert("verify_feedback".to_string(), Value::String(reason));
                g.insert("action".to_string(), Value::String("reimplement".to_string()));
            }
            drop(g);
            store
        })
    });

    let workflow = RpiWorkflow::new()
        .with_research(research)
        .with_plan(plan)
        .with_implement(implement)
        .with_verify(verify);

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    store.write().await.insert("goal".to_string(), Value::String(goal.to_string()));

    println!("=== RPI Workflow ===");
    println!("Goal: {}\n", goal);

    let final_store = workflow.run(store).await;
    let g = final_store.read().await;

    println!("\n=== Final Implementation ===\n");
    println!("{}", g.get("code").and_then(|v| v.as_str()).unwrap_or("(no code produced)"));
}
