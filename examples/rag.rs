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
use std::sync::{Arc, Mutex};

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
            let query = store
                .lock()
                .unwrap()
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let retrieval_prompt = format!(
                "You are a search assistant. Given the user query: '{}', retrieve or synthesize a concise context from your knowledge base or the web that would help answer the question.",
                query
            );
            println!("\n[Retriever Phase]");
            println!("Retrieval prompt:\n{}\n", retrieval_prompt);

            let client = providers::openai::Client::from_env();
            let rig_agent = client.agent("gpt-4.1-mini")
                .preamble("You are a helpful retrieval agent.")
                .build();

            let context = match rig_agent.prompt(&retrieval_prompt).await {
                Ok(resp) => resp,
                Err(e) => format!("Error: {}", e),
            };

            println!("Retrieved context:\n{}\n", context);

            store.lock().unwrap().insert("context".to_string(), Value::String(context));
            store
        })
    });

    // Generator: Use rig to generate an answer based on the retrieved context
    let generator = create_node(|store: SharedStore| {
        Box::pin(async move {
            let query = store
                .lock()
                .unwrap()
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let context = store
                .lock()
                .unwrap()
                .get("context")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let generation_prompt = format!(
                "You are an expert assistant. Given the user query: '{}', and the following context:\n{}\n\nGenerate a clear, concise, and accurate answer for the user.",
                query, context
            );
            println!("[Generator Phase]");
            println!("Generation prompt:\n{}\n", generation_prompt);

            let client = providers::openai::Client::from_env();
            let rig_agent = client.agent("gpt-3.5-turbo")
                .preamble("You are an expert answer generator.")
                .build();

            let response = match rig_agent.prompt(&generation_prompt).await {
                Ok(resp) => resp,
                Err(e) => format!("Error: {}", e),
            };

            println!("Generated answer:\n{}\n", response);

            store.lock().unwrap().insert("response".to_string(), Value::String(response));
            store
        })
    });

    // Compose the RAG pipeline
    let rag = Rag::new(retriever, generator);

    // Run the RAG pipeline
    let result = rag.call(Arc::new(Mutex::new(store))).await;
    let locked = result.lock().unwrap();

    println!("=== RAG Flow Complete ===");
    println!("User Query: {}", user_query);
    if let Some(context) = locked.get("context").and_then(|v| v.as_str()) {
        println!("\n[Final Retrieved Context]\n{}", context);
    }
    if let Some(response) = locked.get("response").and_then(|v| v.as_str()) {
        println!("\n[Final Generated Answer]\n{}", response);
    }
}
