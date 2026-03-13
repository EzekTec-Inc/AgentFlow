/*!
# Example: agent.rs

**Purpose:**
Demonstrates how to create a single LLM-powered agent using AgentFlow and the rig crate, including retry logic and both ergonomic and low-level usage.

**How it works:**
- Defines a node that takes a prompt from the store and calls an LLM (via rig) to generate a response.
- Wraps the node in an `Agent` with retry logic.
- Shows both the high-level `decide` method (HashMap in/out) and the lower-level `call` method (SharedStore in/out).

**How to adapt:**
- Change the prompt or LLM model to suit your use case.
- Use `Agent::with_retry` to add robustness to any LLM or tool call.
- Use `decide` for ergonomic, single-step agent calls in your own projects.

**Example:**
```rust
let agent = Agent::with_retry(my_node, 3, 1000);
let result = agent.decide(my_input).await;
```
*/

use agentflow::prelude::*;
use dotenvy::dotenv;
use rig::prelude::*;
use rig::{completion::Prompt, providers};
use serde_json::Value;
use std::collections::HashMap;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() {
    dotenv().ok();
    fmt()
        .with_env_filter(EnvFilter::new("agentflow=debug,agent=debug"))
        .init();

    let example_prompt = "Write a concise and summarized ode to ai in shakespeare";
    let mut store = HashMap::new();
    store.insert(
        "prompt".to_string(),
        Value::String(example_prompt.to_string()),
    );

    println!("[rig-core prompt]: \n{}\n", example_prompt);

    let agent_node = create_node(move |store: SharedStore| {
        Box::pin(async move {
            let prompt = {
                let guard = store.write().await;
                guard
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };

            let openai_client = providers::openai::Client::from_env();
            let rig_agent = openai_client
                .agent("gpt-4o-mini")
                .preamble(r#"You are a helpful assistant who is very skilled at writing poetry."#)
                .build();

            let response = match rig_agent.prompt(&prompt).await {
                Ok(llm_response) => llm_response,
                Err(e) => format!("Error: {}", e),
            };

            store
                .write()
                .await
                .insert("response".to_string(), Value::String(response));
            store
        })
    });

    let agent = Agent::with_retry(agent_node, 3, 1500);

    let result = agent.decide(store.clone()).await;

    if let Some(response) = result.get("response").and_then(|v| v.as_str()) {
        println!("[llm response]: \n{}\n", response);
    }

    let shared_store = std::sync::Arc::new(tokio::sync::RwLock::new(store));
    let result_store = agent.call(shared_store).await;
    let result_map = std::sync::Arc::try_unwrap(result_store).map_or_else(
        |arc| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async { arc.write().await.clone() })
        },
        |mutex| mutex.into_inner(),
    );

    if let Some(response) = result_map.get("response").and_then(|v| v.as_str()) {
        println!("[direct call response]: \n{}\n", response);
    }
}
