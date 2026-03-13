/*!
# Example: orchestrator_multi_agent.rs

**Purpose:**
Demonstrates an orchestrator agent coordinating a multi-phase, multi-role workflow (research, code, review) with real LLM calls and user progress updates.

**How it works:**
- Each phase is a separate LLM agent.
- The orchestrator runs each phase in sequence, passing real data between them.
- Progress is displayed at each step, and the final report is aggregated and shown.

**How to adapt:**
- Use this pattern for any orchestrated, multi-phase workflow (e.g., document processing, multi-stage approval, content generation).
- Add more phases or change the logic as needed.

**Example:**
```rust
let orchestrator_node = create_node(move |store| { ... });
let agent = Agent::new(orchestrator_node);
let result = agent.decide(store).await;
```
*/

use agentflow::prelude::*;
use rig::prelude::*;
use rig::{completion::Prompt, providers};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Helper to safely extract a string from the store
async fn get_string_from_store(store: &SharedStore, key: &str) -> String {
    let guard = store.write().await;
    guard
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Helper to create a rig-instrumented LLM node for a given model, preamble, and output key
fn llm_agent_node<F>(
    client: providers::openai::Client,
    model: &str,
    preamble: &str,
    output_key: &'static str,
    prompt_generator: F,
) -> SimpleNode
where
    F: Fn(SharedStore) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>>
        + Send
        + Sync
        + 'static,
{
    let model = model.to_string();
    let preamble = preamble.to_string();
    let output_key = output_key.to_string();
    let prompt_generator = Arc::new(prompt_generator);

    create_node(move |store: SharedStore| {
        let client = client.clone();
        let model = model.clone();
        let preamble = preamble.clone();
        let output_key = output_key.clone();
        let prompt_generator = Arc::clone(&prompt_generator);

        Box::pin(async move {
            let prompt = prompt_generator(store.clone()).await;

            println!("Starting phase: {}", output_key);
            sleep(Duration::from_millis(500)).await;

            let rig_agent = client.agent(&model).preamble(&preamble).build();

            let response = match rig_agent.prompt(&prompt).await {
                Ok(resp) => resp,
                Err(e) => format!("Error: {}", e),
            };

            store
                .write()
                .await
                .insert(output_key.clone(), Value::String(response));
            println!("Completed phase: {}", output_key);
            store
        })
    })
}

#[tokio::main]
async fn main() {
    let topic = "Rust AgentFlow Framework";
    let mut initial_store = HashMap::new();
    initial_store.insert("topic".to_string(), Value::String(topic.to_string()));

    let client = providers::openai::Client::from_env();

    // Research node: generates facts
    let research_node = llm_agent_node(
        client.clone(),
        "gpt-4o-mini",
        "You are a research assistant.",
        "research_facts",
        move |store| {
            Box::pin(async move {
                let topic = get_string_from_store(&store, "topic").await;
                format!(
                    "Research and summarize 5 key facts about {} for a software project. Output as a numbered list.",
                    topic
                )
            })
        },
    );

    // Code node: uses facts from research phase
    let code_node = llm_agent_node(
        client.clone(),
        "gpt-4o-mini",
        "You are a senior TypeScript developer.",
        "typescript_code",
        move |store| {
            Box::pin(async move {
                let facts = get_string_from_store(&store, "research_facts").await;
                format!(
                    "Write a TypeScript function that prints key facts identified, chosen from the following list:\n{}\nOutput only the TypeScript code.",
                    facts
                )
            })
        },
    );

    // Review node: reviews the code generated in the code phase
    let review_node = llm_agent_node(
        client.clone(),
        "gpt-4o-mini",
        "You are a code reviewer.",
        "review",
        move |store| {
            Box::pin(async move {
                let code = get_string_from_store(&store, "typescript_code").await;
                format!(
                    "Review the following TypeScript code for correctness and style. Suggest improvements if needed.\n\n{}",
                    code
                )
            })
        },
    );

    // Orchestrator node: runs each phase in sequence, passing real data between them
    let orchestrator_node = create_node(move |store: SharedStore| {
        let research_node = research_node.clone();
        let code_node = code_node.clone();
        let review_node = review_node.clone();

        Box::pin(async move {
            let mut report = String::from("🎯 Orchestrator Report\n");

            // Research phase
            let store = research_node.call(store).await;
            let facts = get_string_from_store(&store, "research_facts").await;
            if facts.starts_with("Error:") {
                println!("Workflow halted: Research phase failed.");
                return store;
            }

            // Code phase
            let store = code_node.call(store).await;
            let code = get_string_from_store(&store, "typescript_code").await;
            if code.starts_with("Error:") {
                println!("Workflow halted: Code phase failed.");
                return store;
            }

            // Review phase
            let store = review_node.call(store).await;
            let review = get_string_from_store(&store, "review").await;
            if review.starts_with("Error:") {
                println!("Workflow halted: Review phase failed.");
                return store;
            }

            report.push_str("\n--- 1. Research Facts ---\n");
            report.push_str(&facts);
            report.push_str("\n\n--- 2. Generated TypeScript ---\n");
            report.push_str(&code);
            report.push_str("\n\n--- 3. Code Review ---\n");
            report.push_str(&review);

            println!("\n{}\n", report);

            store
        })
    });

    let agent = Agent::new(orchestrator_node);
    let _ = agent.decide(initial_store).await;
}
