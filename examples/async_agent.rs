/*!
# Example: async_agent.rs

**Purpose:**
Showcases running multiple agents concurrently (async/parallel) using PocketFlow and rig.

**How it works:**
- Defines two LLM nodes with different prompts.
- Wraps each in an `Agent`.
- Runs both agents concurrently using `tokio::join!`.
- Prints both prompts and both responses.

**How to adapt:**
- Use this pattern to parallelize LLM calls (e.g., for batch processing, multi-agent chat, or tool use).
- Add more agents or change the prompts/models as needed.

**Example:**
```rust
let fut1 = agent1.decide(input1);
let fut2 = agent2.decide(input2);
let (result1, result2) = tokio::join!(fut1, fut2);
```
*/

use agentflow::prelude::*;
use rig::prelude::*;
use rig::{completion::Prompt, providers};
use serde_json::Value;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    // Prepare input for two agents
    let mut store1 = HashMap::new();
    store1.insert("prompt".to_string(), Value::String("Write a haiku about async Rust.".to_string()));

    let mut store2 = HashMap::new();
    store2.insert("prompt".to_string(), Value::String("Summarize the benefits of concurrency.".to_string()));

    // Create two rig-instrumented async LLM nodes
    let llm_node = |desc: &'static str| {
        create_node(move |store: SharedStore| {
            Box::pin(async move {
                // Simulate network/LLM latency
                sleep(Duration::from_millis(500)).await;

                let prompt = store
                    .lock()
                    .unwrap()
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // Use rig-core to call an LLM (OpenAI, etc.)
                let openai_client = providers::openai::Client::from_env();
                let rig_agent = openai_client
                    .agent("gpt-4.1-mini")
                    .preamble(&format!("You are a helpful assistant for {}.", desc))
                    .build();

                let response = match rig_agent.prompt(&prompt).await {
                    Ok(llm_response) => llm_response,
                    Err(e) => format!("Error: {}", e),
                };

                store.lock().unwrap().insert("response".to_string(), Value::String(response));
                store
            })
        })
    };

    // Emmitting the prompts used
    println!("Agent 1 (poetry) prompt: {}\n", &store1.get("prompt").unwrap());
    println!("Agent 2 (summarization) prompt: {}\n", &store2.get("prompt").unwrap());
    println!("====================================================================\n");

    // Wrap each node in an Agent
    let agent1 = Agent::with_retry(llm_node("poetry"), 2, 500);
    let agent2 = Agent::with_retry(llm_node("summarization"), 2, 500);

    // Run both agents concurrently (showcasing async power)
    let fut1 = agent1.decide(store1);
    let fut2 = agent2.decide(store2);


    let (result1, result2) = tokio::join!(fut1, fut2);

    // Print results
    println!("Agent 1 (poetry) response:\n{}\n", result1.get("response").unwrap_or(&Value::Null));
    println!("Agent 2 (summarization) response:\n{}\n", result2.get("response").unwrap_or(&Value::Null));
}
