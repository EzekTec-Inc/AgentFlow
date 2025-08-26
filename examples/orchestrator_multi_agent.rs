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
// Removed unused imports Arc and Mutex
use tokio::time::{sleep, Duration};

/// Helper to create a rig-instrumented LLM node for a given model, preamble, and prompt key
fn llm_agent_node(
    model: &str,
    preamble: &str,
    prompt_key: &'static str,
    output_key: &'static str,
) -> SimpleNode {
    let model = model.to_string();
    let preamble = preamble.to_string();
    let output_key = output_key.to_string();
    create_node(move |store: SharedStore| {
        let prompt = store
            .lock()
            .unwrap()
            .get(prompt_key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Box::pin({
            let model = model.clone();
            let preamble = preamble.clone();
            let output_key = output_key.clone();
            async move {
                println!("Starting phase: {}", output_key);
                sleep(Duration::from_millis(500)).await;

                let client = providers::openai::Client::from_env();
                let rig_agent = client.agent(&model).preamble(&preamble).build();

                let response = match rig_agent.prompt(&prompt).await {
                    Ok(resp) => resp,
                    Err(e) => format!("Error: {}", e),
                };

                store.lock().unwrap().insert(output_key.clone(), Value::String(response));
                println!("Completed phase: {}", output_key);
                store
            }
        })
    })
}

#[tokio::main]
async fn main() {
    let topic = "maple syrup";
    let mut store = HashMap::new();
    store.insert("topic".to_string(), Value::String(topic.to_string()));

    // Prepare the research prompt
    let research_prompt = format!(
        "You are a research assistant. Research and summarize 5 key facts about {} for a software project. Output as a numbered list.",
        topic
    );
    store.insert("research_prompt".to_string(), Value::String(research_prompt));

    // Research node: generates facts
    let research_node = llm_agent_node(
        "gpt-4.1-mini",
        "You are a research assistant.",
        "research_prompt",
        "research_facts"
    );

    // Code node: uses facts from research phase
    let code_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            println!("Starting phase: code");
            sleep(Duration::from_millis(500)).await;

            let facts = store
                .lock()
                .unwrap()
                .get("research_facts")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let code_prompt = format!(
                "You are a senior TypeScript developer. Write a TypeScript function that prints one fun fact about maple syrup, chosen from the following list:\n{}\nOutput only the TypeScript code.",
                facts
            );

            let client = providers::openai::Client::from_env();
            let rig_agent = client
                .agent("gpt-3.5-turbo")
                .preamble("You are a senior TypeScript developer.")
                .build();

            let response = match rig_agent.prompt(&code_prompt).await {
                Ok(resp) => resp,
                Err(e) => format!("Error: {}", e),
            };

            store.lock().unwrap().insert("typescript_code".to_string(), Value::String(response));
            println!("Completed phase: code");
            store
        })
    });

    // Review node: reviews the code generated in the code phase
    let review_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            println!("Starting phase: review");
            sleep(Duration::from_millis(500)).await;

            let code = store
                .lock()
                .unwrap()
                .get("typescript_code")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let review_prompt = format!(
                "You are a code reviewer. Review the following TypeScript code for correctness and style. Suggest improvements if needed.\n\n{}",
                code
            );

            let client = providers::openai::Client::from_env();
            let rig_agent = client
                .agent("gpt-3.5-turbo")
                .preamble("You are a code reviewer.")
                .build();

            let response = match rig_agent.prompt(&review_prompt).await {
                Ok(resp) => resp,
                Err(e) => format!("Error: {}", e),
            };

            store.lock().unwrap().insert("review".to_string(), Value::String(response));
            println!("Completed phase: review");
            store
        })
    });

    // Orchestrator node: runs each phase in sequence, passing real data between them
    let orchestrator_node = create_node(move |store: SharedStore| {
        let research_node = research_node.clone();
        let code_node = code_node.clone();
        let review_node = review_node.clone();
        Box::pin(async move {
            let mut report = String::from("🎯 Orchestrator Report\n");

            // Research phase
            let store = research_node.call(store).await;
            let facts = store.lock().unwrap().get("research_facts").cloned();

            // Code phase
            let store = code_node.call(store).await;
            let code = store.lock().unwrap().get("typescript_code").cloned();

            // Review phase
            let store = review_node.call(store).await;
            let review = store.lock().unwrap().get("review").cloned();

            // Aggregate results
            if let Some(Value::String(f)) = facts {
                report.push_str(&format!("📚 Research Facts:\n{}\n\n", f));
            }
            if let Some(Value::String(c)) = code {
                report.push_str(&format!("🧑‍💻 TypeScript Code:\n{}\n\n", c));
            }
            if let Some(Value::String(rv)) = review {
                report.push_str(&format!("🔍 Review:\n{}\n\n", rv));
            }
            report.push_str("✅ All phases complete.");

            store.lock().unwrap().insert("report".to_string(), Value::String(report));
            store
        })
    });

    let agent = Agent::new(orchestrator_node);
    let result = agent.decide(store).await;

    if let Some(output) = result.get("report").and_then(|v| v.as_str()) {
        println!("\n{}", output);
    }
}
