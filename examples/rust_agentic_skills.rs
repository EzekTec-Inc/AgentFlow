/*!
# Example: rust_agentic_skills.rs

Real-world RPI workflow driven by a Skill file and real LLM calls. The skill
defines the agent's persona and instructions. Each RPI phase (Research, Plan,
Implement, Verify) calls the LLM with that context.

Domain: generating a Rust CLI tool from a spec using the Skill system.

Requires: OPENAI_API_KEY, `skills` feature
Run with: cargo run --example rust-agentic-skills --features skills
*/

use agentflow::core::error::AgentFlowError;
use agentflow::core::node::{create_node, SharedStore};
use agentflow::patterns::rpi::RpiWorkflow;
use agentflow::skills::Skill;
use agentflow::utils::tool::create_tool_node;
use agentflow::core::flow::Flow;
use dotenvy::dotenv;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{fmt, EnvFilter};
use rig::prelude::*;
use rig::{completion::Prompt, providers};

async fn llm_with_skill(skill_instructions: &str, role: &str, user: &str) -> String {
    let system = format!("{}\n\nYour current role: {}", skill_instructions, role);
    let client = providers::openai::Client::from_env();
    let agent  = client.agent("gpt-4o-mini").preamble(&system).build();
    match agent.prompt(user).await {
        Ok(r)  => r,
        Err(e) => format!("LLM error: {e}"),
    }
}

#[tokio::main]
async fn main() -> Result<(), AgentFlowError> {
    dotenv().ok();
    fmt().with_env_filter(EnvFilter::new("agentflow=debug,rust_agentic_skills=debug")).init();

    println!("=== Rust Agentic Skills ===\n");

    // ── Load skill ─────────────────────────────────────────────────────────
    let skill_content = r#"---
name: Rust CLI Generator
description: Generates a minimal Rust CLI tool from a plain-English spec
version: 1.0.0
---
You are an expert Rust developer focused on writing clean, idiomatic CLI tools.
Use the clap crate for argument parsing. Output only Rust code — no markdown
fences, no explanations unless explicitly asked.
"#;

    let skill = Skill::parse(skill_content)?;
    println!("Skill: {} v{}", skill.name, skill.version.as_deref().unwrap_or("?"));
    println!("Description: {}\n", skill.description);

    let instructions = skill.instructions.clone();

    // ── Store setup ────────────────────────────────────────────────────────
    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    let spec = "A CLI tool called `wordfreq` that reads a text file path from the \
                command line and prints the top-10 most frequent words with their counts.";
    store.write().await.insert("spec".to_string(), Value::String(spec.to_string()));

    println!("Spec: {}\n", spec);

    // ── Research ────────────────────────────────────────────────────────────
    let inst = instructions.clone();
    let research = create_node(move |s: SharedStore| {
        let inst = inst.clone();
        Box::pin(async move {
            let spec = s.read().await.get("spec").and_then(|v| v.as_str()).unwrap_or("").to_string();
            println!("[Research] Identifying relevant crates and patterns…");
            let output = llm_with_skill(
                &inst,
                "Researcher: identify the Rust crates and stdlib features needed for this spec. Be brief.",
                &spec,
            ).await;
            println!("[Research]\n{}\n", output.trim());
            s.write().await.insert("research".to_string(), Value::String(output));
            s.write().await.insert("action".to_string(),   Value::String("default".to_string()));
            s
        })
    });

    // ── Plan ───────────────────────────────────────────────────────────────
    let inst = instructions.clone();
    let plan = create_node(move |s: SharedStore| {
        let inst = inst.clone();
        Box::pin(async move {
            let (spec, research) = {
                let g = s.read().await;
                (
                    g.get("spec").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    g.get("research").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                )
            };
            println!("[Plan] Producing implementation steps…");
            let output = llm_with_skill(
                &inst,
                "Planner: given the spec and research, list numbered implementation steps (no code).",
                &format!("Spec: {}\n\nResearch:\n{}", spec, research),
            ).await;
            println!("[Plan]\n{}\n", output.trim());
            s.write().await.insert("plan".to_string(),   Value::String(output));
            s.write().await.insert("action".to_string(), Value::String("default".to_string()));
            s
        })
    });

    // ── Implement ──────────────────────────────────────────────────────────
    let inst = instructions.clone();
    let implement = create_node(move |s: SharedStore| {
        let inst = inst.clone();
        Box::pin(async move {
            let (spec, plan, feedback) = {
                let g = s.read().await;
                (
                    g.get("spec").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    g.get("plan").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    g.get("verify_feedback").and_then(|v| v.as_str()).map(|s| s.to_string()),
                )
            };
            let user = match &feedback {
                Some(fb) => format!("Spec: {}\nPlan:\n{}\n\nFix based on feedback: {}", spec, plan, fb),
                None     => format!("Spec: {}\nPlan:\n{}\n\nWrite the code now.", spec, plan),
            };
            println!("[Implement] Writing Rust code…");
            let code = llm_with_skill(&inst, "Implementer: write the Rust source code.", &user).await;
            println!("[Implement] {} chars of code generated.\n", code.len());
            s.write().await.insert("code".to_string(),   Value::String(code));
            s.write().await.insert("action".to_string(), Value::String("default".to_string()));
            s
        })
    });

    // ── Verify ─────────────────────────────────────────────────────────────
    let inst = instructions.clone();
    let verify = create_node(move |s: SharedStore| {
        let inst = inst.clone();
        Box::pin(async move {
            let (spec, code) = {
                let g = s.read().await;
                (
                    g.get("spec").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    g.get("code").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                )
            };
            println!("[Verify] Code-reviewing implementation…");
            let verdict = llm_with_skill(
                &inst,
                "Reviewer: check that the code matches the spec. \
                 Respond ONLY with PASS or FAIL: <one-sentence reason>.",
                &format!("Spec: {}\n\nCode:\n{}", spec, code),
            ).await;
            println!("[Verify] {}\n", verdict.trim());

            let mut g = s.write().await;
            if verdict.trim().starts_with("PASS") {
                g.remove("verify_feedback");
                g.insert("action".to_string(), Value::String("done".to_string()));
            } else {
                let reason = verdict.trim().strip_prefix("FAIL:").unwrap_or("").trim().to_string();
                g.insert("verify_feedback".to_string(), Value::String(reason));
                g.insert("action".to_string(), Value::String("reimplement".to_string()));
            }
            drop(g);
            s
        })
    });

    // ── Assemble & run ─────────────────────────────────────────────────────
    let workflow = RpiWorkflow::new()
        .with_research(research)
        .with_plan(plan)
        .with_implement(implement)
        .with_verify(verify);

    let store_after_rpi = workflow.run(store).await;

    // ── Optionally echo the generated code via tool node ───────────────────
    let code_snippet = store_after_rpi.read().await
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .lines()
        .take(3)
        .collect::<Vec<_>>()
        .join(" | ");

    store_after_rpi.write().await.insert(
        "echo_input".to_string(),
        Value::String(format!("First 3 lines: {}", code_snippet)),
    );

    let echo_tool = create_tool_node("echo_tool", "echo", vec!["Skill workflow complete!".to_string()]);
    let mut tool_flow = Flow::new();
    tool_flow.add_node("echo", echo_tool);
    let final_store = tool_flow.run(store_after_rpi).await;

    // ── Print results ──────────────────────────────────────────────────────
    let g = final_store.read().await;
    println!("=== Generated Code ===\n");
    println!("{}", g.get("code").and_then(|v| v.as_str()).unwrap_or("(no code)"));

    if let Some(stdout) = g.get("echo_tool_stdout") {
        println!("\nTool stdout: {}", stdout.as_str().unwrap_or("").trim());
    }

    Ok(())
}
