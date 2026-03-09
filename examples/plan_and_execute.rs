/*!
# Example: plan_and_execute.rs

Real-world Plan-and-Execute agent. A Planner LLM breaks a high-level goal into
numbered steps. An Executor LLM processes each step in turn, popping it off the
plan and producing a result. When the plan is empty the flow terminates.

Domain: writing a short technical report on a user-supplied topic.

Requires: OPENAI_API_KEY
Run with: cargo run --example plan-and-execute
*/

use agentflow::core::error::AgentFlowError;
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

const PLANNER_SYSTEM: &str =
    "You are a planning assistant. Given a goal, output a numbered list of concrete, \
     specific tasks needed to accomplish it. Each task must be on its own line, \
     starting with its number and a period (e.g. '1. Do X'). \
     Output only the numbered list — no headings, no extra text.";

const EXECUTOR_SYSTEM: &str =
    "You are a skilled assistant that executes tasks concisely. \
     Given a task description, perform it and output only the result. \
     Be concise but complete — 2-4 sentences per task.";

/// Parse LLM output into a Vec of task strings, stripping numbering.
fn parse_plan(raw: &str) -> Vec<String> {
    raw.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| {
            // Strip leading "1. " / "1) " style numbering
            if let Some(pos) = l.find(". ") {
                let prefix = &l[..pos];
                if prefix.chars().all(|c| c.is_ascii_digit()) {
                    return l[pos + 2..].to_string();
                }
            }
            l.to_string()
        })
        .collect()
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    fmt().with_env_filter(EnvFilter::new("agentflow=debug,plan_and_execute=debug")).init();

    // 1 planner step + N executor steps. 20 is safe for plans up to 19 tasks.
    let mut flow = Flow::new().with_max_steps(20);

    // ── Planner: LLM generates the task list ─────────────────────────────────
    let planner = create_node(|store: SharedStore| {
        Box::pin(async move {
            let goal = {
                let g = store.read().await;
                g.get("goal").and_then(|v| v.as_str()).unwrap_or("").to_string()
            };

            println!("[Planner] Goal: {}", goal);
            println!("[Planner] Generating plan…");

            let client = providers::openai::Client::from_env();
            let agent  = client.agent("gpt-4o-mini").preamble(PLANNER_SYSTEM).build();

            let raw = match agent.prompt(&goal).await {
                Ok(r)  => r,
                Err(e) => { eprintln!("[Planner] LLM error: {e}"); String::new() }
            };

            let tasks = parse_plan(&raw);
            println!("[Planner] {} tasks generated:", tasks.len());
            for (i, t) in tasks.iter().enumerate() { println!("  {}. {}", i + 1, t); }

            let plan_json: Vec<Value> = tasks.into_iter().map(Value::String).collect();

            let mut g = store.write().await;
            g.insert("plan".to_string(),    Value::Array(plan_json));
            g.insert("results".to_string(), Value::Array(vec![]));
            g.insert("action".to_string(),  Value::String("execute".to_string()));
            drop(g);
            store
        })
    });

    // ── Executor: pops one task, calls LLM, stores result ────────────────────
    let executor = create_node(|store: SharedStore| {
        Box::pin(async move {
            let (task, remaining) = {
                let g = store.read().await;
                match g.get("plan") {
                    Some(Value::Array(arr)) if !arr.is_empty() => {
                        let task = arr[0].as_str().unwrap_or("").to_string();
                        let rest = arr[1..].to_vec();
                        (Some(task), rest)
                    }
                    _ => (None, vec![]),
                }
            };

            let Some(task) = task else {
                // Plan is empty — signal completion
                store.write().await.insert("action".to_string(), Value::String("done".to_string()));
                return store;
            };

            println!("\n[Executor] Task: {}", task);

            let client = providers::openai::Client::from_env();
            let agent  = client.agent("gpt-4o-mini").preamble(EXECUTOR_SYSTEM).build();

            let result = match agent.prompt(&task).await {
                Ok(r)  => r,
                Err(e) => format!("Error executing task: {e}"),
            };
            println!("[Executor] Result: {}", result.trim());

            let mut g = store.write().await;

            // Update plan (pop front)
            g.insert("plan".to_string(), Value::Array(remaining));

            // Append result
            if let Some(Value::Array(ref mut results)) = g.get_mut("results") {
                results.push(Value::String(format!("**{}**\n{}", task, result.trim())));
            }

            // Keep looping if tasks remain, else signal done
            let still_tasks = g.get("plan")
                .and_then(|v| v.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false);

            g.insert(
                "action".to_string(),
                Value::String(if still_tasks { "execute" } else { "done" }.to_string()),
            );
            drop(g);
            store
        })
    });

    flow.add_node("planner",  planner);
    flow.add_node("executor", executor);
    flow.add_edge("planner",  "execute", "executor");
    flow.add_edge("executor", "execute", "executor"); // self-loop while tasks remain
    // "done" has no edge → flow stops naturally

    let goal = "Write a short technical report on how Rust prevents memory safety bugs";

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    store.write().await.insert("goal".to_string(), Value::String(goal.to_string()));

    println!("=== Plan-and-Execute Agent ===");
    println!("Goal: {}\n", goal);

    match flow.run_safe(store).await {
        Ok(s) => {
            let g = s.read().await;
            println!("\n=== Final Report ===\n");
            if let Some(Value::Array(results)) = g.get("results") {
                for section in results {
                    println!("{}\n", section.as_str().unwrap_or(""));
                }
            }
        }
        Err(AgentFlowError::ExecutionLimitExceeded(msg)) => eprintln!("Step limit hit: {msg}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}
