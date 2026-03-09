/*!
# Example: orchestrator_with_tools.rs

Real-world orchestrator that delegates to a ReAct sub-agent. The sub-agent
uses a real shell tool (`uname -a`) and passes the result back to the
Orchestrator LLM, which then writes a human-readable system summary.

How it works:
1. Orchestrator (LLM) receives the main task and delegates to the ReAct flow.
2. ReAct Reasoner (LLM) decides to call the `sysinfo` tool.
3. Tool executor runs `uname -a` via the built-in create_tool_node.
4. ReAct Reasoner (LLM) reads the tool output and produces a final answer.
5. Orchestrator (LLM) formats the answer into a polished report.

Requires: OPENAI_API_KEY
Run with: cargo run --example orchestrator-with-tools
*/

use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use agentflow::utils::tool::create_tool_node;
use dotenvy::dotenv;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{fmt, EnvFilter};
use rig::prelude::*;
use rig::{completion::Prompt, providers};

const REASONER_SYSTEM: &str =
    "You are a system-info assistant. You have one tool:\n\
     - sysinfo: runs `uname -a` and returns the output.\n\
     Respond with EXACTLY one of:\n\
       ACTION: sysinfo\n\
       ANSWER: <your final answer>\n\
     No other text.";

const ORCHESTRATOR_SYSTEM: &str =
    "You are a technical report writer. Given a raw sub-agent answer about the current system, \
     write a brief, friendly 3-sentence summary a non-technical user can understand.";

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
    fmt().with_env_filter(EnvFilter::new("agentflow=debug,orchestrator_with_tools=debug")).init();

    println!("=== Orchestrator with Tool-Using Sub-Agent ===\n");

    // ── Main orchestrator node ────────────────────────────────────────────────
    let orchestrator = create_node(|store: SharedStore| {
        Box::pin(async move {
            let task = store.read().await
                .get("main_task")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            println!("[Orchestrator] Task: {}", task);
            println!("[Orchestrator] Delegating to ReAct sub-agent…\n");

            // ── Build the ReAct sub-flow ──────────────────────────────────────
            let mut react = Flow::new().with_max_steps(10);

            let reasoner = create_node(|s: SharedStore| {
                Box::pin(async move {
                    let (task, tool_out) = {
                        let g = s.read().await;
                        (
                            g.get("sub_task").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            g.get("sysinfo_stdout").and_then(|v| v.as_str()).map(|x| x.to_string()),
                        )
                    };

                    let user_msg = match &tool_out {
                        Some(out) => format!("Task: {}\n\nTool output:\n{}", task, out),
                        None      => format!("Task: {}", task),
                    };

                    println!("[ReAct Reasoner] Thinking…");
                    let reply = llm(REASONER_SYSTEM, &user_msg).await;
                    println!("[ReAct Reasoner] {}", reply.trim());

                    let mut g = s.write().await;
                    if reply.trim().starts_with("ANSWER:") {
                        let ans = reply.trim().strip_prefix("ANSWER:").unwrap_or("").trim().to_string();
                        g.insert("sub_answer".to_string(), Value::String(ans));
                        g.insert("action".to_string(),     Value::String("done".to_string()));
                    } else {
                        // ACTION: sysinfo
                        g.insert("action".to_string(), Value::String("use_tool".to_string()));
                    }
                    drop(g);
                    s
                })
            });

            // Real tool: runs `uname -a`, output stored as `sysinfo_stdout`
            let tool = create_tool_node("sysinfo", "uname", vec!["-a".to_string()]);

            react.add_node("reasoner", reasoner);
            react.add_node("tool",     tool);
            react.add_edge("reasoner", "use_tool", "tool");
            react.add_edge("tool",     "use_tool", "reasoner"); // tool keeps action="use_tool"

            // Inject sub-task into store
            store.write().await.insert(
                "sub_task".to_string(),
                Value::String("Find out the current operating system and kernel version.".to_string()),
            );

            let store = react.run(store).await;

            // ── Orchestrator summarises the sub-agent result ──────────────────
            let sub_answer = store.read().await
                .get("sub_answer")
                .and_then(|v| v.as_str())
                .unwrap_or("No answer returned.")
                .to_string();

            println!("\n[Orchestrator] Sub-agent answer: {}", sub_answer);
            println!("[Orchestrator] Writing report…");

            let report = llm(
                ORCHESTRATOR_SYSTEM,
                &format!("Main task: {}\n\nSub-agent answer: {}", task, sub_answer),
            ).await;

            println!("\n[Orchestrator] Report:\n{}", report.trim());
            store.write().await.insert("final_report".to_string(), Value::String(report));
            store
        })
    });

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    store.write().await.insert(
        "main_task".to_string(),
        Value::String("Generate a daily system status report for the ops team.".to_string()),
    );

    let final_store = orchestrator.call(store).await;
    let g = final_store.read().await;
    println!("\n=== Final Report ===\n{}", g.get("final_report").and_then(|v| v.as_str()).unwrap_or(""));
}
