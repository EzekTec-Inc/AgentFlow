/*!
# Example: dynamic_orchestrator.rs

A dynamic orchestrator that reads agent configuration from `examples/agents.toml`
at runtime. If the file does not exist it is created with defaults before proceeding.

How it works:
1. Boot       — load (or create) `examples/agents.toml`, build an AgentRegistry.
2. Planner    — LLM receives the goal + available agent names, returns a JSON array
   of { name, prompt } objects selecting which agents to run and in what order.
3. Dispatcher — pops one AgentSpec per cycle, looks it up in the registry, runs it,
   appends the result; loops until the plan is empty.
4. Aggregator — LLM synthesises every agent result into a final report.

Requires: OPENAI_API_KEY
Run with: cargo run --example dynamic-orchestrator
*/

use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use dotenvy::dotenv;
use rig::prelude::*;
use rig::{completion::Prompt, providers};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{fmt, EnvFilter};

// ── TOML config types ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
struct AgentConfig {
    name: String,
    provider: String,
    model: String,
    preamble: String,
    output_key: String,
}

#[derive(Debug, Deserialize)]
struct AgentsFile {
    agent: Vec<AgentConfig>,
}

// ── Default TOML written when file is absent ──────────────────────────────────

const DEFAULT_TOML: &str = r#"# Dynamic Orchestrator — agent registry configuration.
# Each [[agent]] entry is one modular agent the orchestrator may spin up.
# Supported providers: "openai" | "gemini"

[[agent]]
name        = "researcher"
provider    = "openai"
model       = "gpt-4.1-mini"
preamble    = "You are a concise research assistant. Answer in 3-5 sentences."
output_key  = "research_result"

[[agent]]
name        = "coder"
provider    = "openai"
model       = "gpt-4.1-mini"
preamble    = "You are a senior Rust developer. Produce only clean, compilable Rust code."
output_key  = "code_result"

[[agent]]
name        = "reviewer"
provider    = "openai"
model       = "gpt-4.1-mini"
preamble    = "You are a thorough code reviewer. Be concise; bullet-point your findings."
output_key  = "review_result"
"#;

// ── LLM helper ────────────────────────────────────────────────────────────────

async fn llm_call(provider: &str, model: &str, preamble: &str, user: &str) -> String {
    match provider {
        "gemini" => {
            let client = providers::gemini::Client::from_env();
            let agent = client.agent(model).preamble(preamble).build();
            match agent.prompt(user).await {
                Ok(r) => r,
                Err(e) => format!("LLM error: {e}"),
            }
        }
        _ => {
            let client = providers::openai::Client::from_env();
            let agent = client.agent(model).preamble(preamble).build();
            match agent.prompt(user).await {
                Ok(r) => r,
                Err(e) => format!("LLM error: {e}"),
            }
        }
    }
}

// ── AgentFactory type ─────────────────────────────────────────────────────────

type AgentFactory = Arc<
    dyn Fn(
            String,
            SharedStore,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = SharedStore> + Send>>
        + Send
        + Sync,
>;

// ── Build registry from parsed configs ───────────────────────────────────────

fn build_registry(configs: Vec<AgentConfig>) -> HashMap<String, (AgentFactory, String)> {
    let mut registry: HashMap<String, (AgentFactory, String)> = HashMap::new();

    for cfg in configs {
        let provider = cfg.provider.clone();
        let model = cfg.model.clone();
        let preamble = cfg.preamble.clone();
        let output_key = cfg.output_key.clone();
        let name = cfg.name.clone();

        let factory: AgentFactory = Arc::new(move |prompt: String, store: SharedStore| {
            let provider = provider.clone();
            let model = model.clone();
            let preamble = preamble.clone();
            let output_key = output_key.clone();
            Box::pin(async move {
                println!("[Agent:{}] Running...", output_key);
                let result = llm_call(&provider, &model, &preamble, &prompt).await;
                println!("[Agent:{}] Done.", output_key);
                store
                    .write()
                    .await
                    .insert(output_key.clone(), Value::String(result));
                store
            })
        });

        registry.insert(name, (factory, cfg.output_key.clone()));
    }

    registry
}

// ── Planner system prompt ─────────────────────────────────────────────────────

fn planner_system(agent_names: &[String]) -> String {
    format!(
        "You are an orchestration planner. Given a goal, select which agents to run \
         and in what order from this list: [{}].\n\
         Respond with ONLY a valid JSON array. Each element must be an object with \
         exactly two string keys:\n\
           \"name\"   — one of the agent names listed above\n\
           \"prompt\" — the specific instruction for that agent\n\
         No markdown fences, no explanation, no extra text — raw JSON array only.",
        agent_names.join(", ")
    )
}

const AGGREGATOR_SYSTEM: &str =
    "You are a technical report writer. Synthesise the outputs from multiple agents \
     into a single, well-structured report. Use clear section headings.";

// ── main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    dotenv().ok();
    fmt()
        .with_env_filter(EnvFilter::new("agentflow=debug,dynamic_orchestrator=debug"))
        .init();

    println!("=== Dynamic Orchestrator ===\n");

    // ── Boot: load or create agents.toml ─────────────────────────────────────
    let toml_path = Path::new("examples/agents.toml");

    if !toml_path.exists() {
        println!("[Boot] examples/agents.toml not found — creating with defaults.\n");
        std::fs::write(toml_path, DEFAULT_TOML).expect("Failed to create examples/agents.toml");
    } else {
        println!("[Boot] Loaded examples/agents.toml\n");
    }

    let toml_str = std::fs::read_to_string(toml_path).expect("Failed to read examples/agents.toml");

    let agents_file: AgentsFile =
        toml::from_str(&toml_str).expect("Failed to parse examples/agents.toml");

    let configs = agents_file.agent;
    let agent_names: Vec<String> = configs.iter().map(|c| c.name.clone()).collect();

    println!("[Boot] Available agents: {:?}\n", agent_names);

    let registry = Arc::new(build_registry(configs));

    // ── Build Flow ────────────────────────────────────────────────────────────
    let mut flow = Flow::new().with_max_steps(30);

    // Planner node
    let agent_names_c = agent_names.clone();
    let planner = create_node(move |store: SharedStore| {
        let agent_names = agent_names_c.clone();
        Box::pin(async move {
            let goal = store
                .read()
                .await
                .get("goal")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            println!("[Planner] Goal: {}", goal);
            println!("[Planner] Selecting agents...");

            let system = planner_system(&agent_names);
            let raw = llm_call("openai", "gpt-4.1-mini", &system, &goal).await;
            println!("[Planner] Raw plan: {}", raw.trim());

            let plan: Vec<Value> = serde_json::from_str(raw.trim()).unwrap_or_else(|e| {
                eprintln!("[Planner] JSON parse error: {e} — using empty plan");
                vec![]
            });

            println!("[Planner] {} agent(s) selected.", plan.len());

            let mut g = store.write().await;
            g.insert("agent_plan".to_string(), Value::Array(plan));
            g.insert("agent_results".to_string(), Value::Array(vec![]));
            g.insert("action".to_string(), Value::String("dispatch".to_string()));
            drop(g);
            store
        })
    });

    // Dispatcher node
    let registry_c = Arc::clone(&registry);
    let dispatcher = create_node(move |store: SharedStore| {
        let registry = Arc::clone(&registry_c);
        Box::pin(async move {
            // Pop the first spec from the plan
            let (spec, remaining) = {
                let g = store.read().await;
                match g.get("agent_plan") {
                    Some(Value::Array(arr)) if !arr.is_empty() => {
                        (Some(arr[0].clone()), arr[1..].to_vec())
                    }
                    _ => (None, vec![]),
                }
            };

            let Some(spec) = spec else {
                store
                    .write()
                    .await
                    .insert("action".to_string(), Value::String("aggregate".to_string()));
                return store;
            };

            let agent_name = spec
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let prompt = spec
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            println!("\n[Dispatcher] Running agent: '{}'", agent_name);

            let store = match registry.get(&agent_name) {
                Some((factory, output_key)) => {
                    let store = factory(prompt, store).await;
                    // Append result to agent_results
                    let result = store
                        .read()
                        .await
                        .get(output_key.as_str())
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    {
                        let mut g = store.write().await;
                        if let Some(Value::Array(ref mut results)) = g.get_mut("agent_results") {
                            results.push(Value::String(format!(
                                "### {} ({})\n{}",
                                agent_name, output_key, result
                            )));
                        }
                    }
                    store
                }
                None => {
                    eprintln!("[Dispatcher] Unknown agent '{}' — skipping.", agent_name);
                    store
                }
            };

            // Update plan and loop
            let mut g = store.write().await;
            g.insert("agent_plan".to_string(), Value::Array(remaining));
            g.insert("action".to_string(), Value::String("dispatch".to_string()));
            drop(g);
            store
        })
    });

    // Aggregator node
    let aggregator = create_node(|store: SharedStore| {
        Box::pin(async move {
            let (goal, results_text) = {
                let g = store.read().await;
                let goal = g
                    .get("goal")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let text = match g.get("agent_results") {
                    Some(Value::Array(arr)) => arr
                        .iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                    _ => String::new(),
                };
                (goal, text)
            };

            println!("\n[Aggregator] Synthesising final report...");

            let user_msg = format!(
                "Original goal: {}\n\nAgent outputs:\n{}",
                goal, results_text
            );
            let report = llm_call("openai", "gpt-4.1-mini", AGGREGATOR_SYSTEM, &user_msg).await;

            println!("[Aggregator] Report ready.");
            store
                .write()
                .await
                .insert("final_report".to_string(), Value::String(report));
            // No action set — Flow stops naturally
            store
        })
    });

    flow.add_node("planner", planner);
    flow.add_node("dispatcher", dispatcher);
    flow.add_node("aggregator", aggregator);
    flow.add_edge("planner", "dispatch", "dispatcher");
    flow.add_edge("dispatcher", "dispatch", "dispatcher"); // self-loop per agent
    flow.add_edge("dispatcher", "aggregate", "aggregator");

    // ── Run ───────────────────────────────────────────────────────────────────
    let goal =
        "Build a Rust function that fetches a URL and returns the response body as a String.";

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    store
        .write()
        .await
        .insert("goal".to_string(), Value::String(goal.to_string()));

    println!("Goal: {}\n", goal);

    let final_store = flow.run(store).await;
    let g = final_store.read().await;

    println!("\n=== Final Report ===\n");
    println!(
        "{}",
        g.get("final_report")
            .and_then(|v| v.as_str())
            .unwrap_or("(no report)")
    );
}
