/*!
# Example: rag.rs

**Purpose:**
Implements a real-world Retrieval-Augmented Generation (RAG) pipeline using rig for both retrieval and generation.

**How it works:**
- The retriever agent uses an LLM to synthesize or retrieve context for a user query.
- The generator agent uses an LLM to generate an answer based on the context.
- The flow and all prompts/results are displayed to the user.

**How to adapt:**
- Replace the retrieval/generation logic with your own (e.g., use a real search API for retrieval).
- Use this pattern for any RAG use case: question answering, summarization, etc.

**Example:**
```rust
let rag = Rag::new(retriever, generator);
let result = rag.call(store).await;
```
*/

use agentflow::prelude::*;
use rig::prelude::*;
use rig::{completion::Prompt, providers};
use serde_json::Value;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    // User query
    let user_query = "What are the main features of Rust for web development?";

    // Prepare the input store
    let mut store = HashMap::new();
    store.insert("query".to_string(), Value::String(user_query.to_string()));

    // Retriever: Use rig to synthesize context
    let retriever = create_node(|store: SharedStore| {
        Box::pin(async move {
            let query = {
                let guard = store.write().await;
                guard
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };

            let retrieval_prompt = format!(
                "You are a search assistant. Given the user query: '{}', retrieve or synthesize a concise context from your knowledge base or the web that would help answer the question.",
                query
            );
            println!("\n[Retriever Phase]");
            println!("Retrieval prompt:\n{}\n", retrieval_prompt);

            let client = providers::openai::Client::from_env();
            let rig_agent = client
                .agent("gpt-4o-mini")
                .preamble(&retrieval_prompt)
                .build();

            let context = match rig_agent
                .prompt("retrieve information based on query given to you in a helpful manner.")
                .await
            {
                Ok(resp) => resp,
                Err(e) => format!("Error: {}", e),
            };

            println!("Retrieved context:\n{}\n", context);

            store
                .write()
                .await
                .insert("context".to_string(), Value::String(context));
            store
        })
    });

    let generator = create_node(|store: SharedStore| {
        Box::pin(async move {
            let (query, context) = {
                let guard = store.write().await;
                let query = guard
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let context = guard
                    .get("context")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                (query, context)
            };

            let generation_prompt = format!(
                "You are an AI assistant. Using the following context:\n\n{}\n\nAnswer the user's query: '{}'",
                context, query
            );
            println!("\n[Generator Phase]");
            println!("Generation prompt:\n{}\n", generation_prompt);

            let client = providers::openai::Client::from_env();
            let rig_agent = client
                .agent("gpt-4o-mini")
                .preamble("You are a helpful answering agent.")
                .build();

            let response = match rig_agent.prompt(&generation_prompt).await {
                Ok(resp) => resp,
                Err(e) => format!("Error: {}", e),
            };

            println!("Generated response:\n{}\n", response);

            store
                .write()
                .await
                .insert("response".to_string(), Value::String(response));
            store
        })
    });

    let rag = Rag::new(retriever, generator);
    let result = rag
        .call(std::sync::Arc::new(tokio::sync::RwLock::new(store)))
        .await;

    let result_map = {
        let guard = result.write().await;
        guard.clone()
    };

    if let Some(response) = result_map.get("response") {
        println!("\n[Final RAG Response]:\n{}", response);
    }
}
