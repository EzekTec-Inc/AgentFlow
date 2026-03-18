/*!
# Example: mapreduce.rs

**Purpose:**
Shows how to use the MapReduce pattern to process a batch of documents, summarize each with an LLM, and aggregate the results.

**How it works:**
- The mapper agent summarizes each document using an LLM.
- The reducer agent concatenates all summaries into a single string.
- The MapReduce pattern handles the orchestration.

**How to adapt:**
- Use this for any batch processing scenario: batch LLM calls, aggregation, analytics.
- Change the mapper/reducer logic to fit your data and goals.

**Example:**
```rust
let map_reduce = MapReduce::new(batch_mapper, reducer);
let result = map_reduce.run(inputs).await;
```
*/

use agentflow::prelude::*;
use rig::prelude::*;
use rig::{completion::Prompt, providers};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

// Implement an enum to hold the keys and values of SharedStore
#[derive(Debug)]
enum SharedStoreKey {
    Doc,
    Summary,
    AllSummaries,
}
impl std::fmt::Display for SharedStoreKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SharedStoreKey::Doc => write!(f, "doc"),
            SharedStoreKey::Summary => write!(f, "summary"),
            SharedStoreKey::AllSummaries => write!(f, "all_summaries"),
        }
    }
}

#[tokio::main]
async fn main() {
    // Prepare a batch of documents as input
    let docs = vec![
        "Rust is a systems programming language.",
        "Async programming enables concurrency.",
        "LLMs are transforming software development.",
    ];

    let inputs: Vec<SharedStore> = docs
        .into_iter()
        .map(|doc| {
            let mut map = HashMap::new();
            map.insert(
                SharedStoreKey::Doc.to_string(),
                Value::String(doc.to_string()),
            );
            Arc::new(tokio::sync::RwLock::new(map))
        })
        .collect();

    // Mapper: Use rig to summarize each document
    let mapper = create_node(|store: SharedStore| {
        Box::pin(async move {
            let doc = {
                let guard = store.write().await;
                guard
                    .get(SharedStoreKey::Doc.to_string().as_str())
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };

            let openai_client = providers::openai::Client::from_env();
            let rig_agent = openai_client
                .agent("gpt-4o-mini")
                .preamble("You are a helpful assistant that summarizes text.")
                .build();

            let prompt = format!("Summarize: {}", doc);
            let summary = match rig_agent.prompt(&prompt).await {
                Ok(resp) => resp,
                Err(e) => format!("Error: {}", e),
            };

            store
                .write()
                .await
                .insert(SharedStoreKey::Summary.to_string(), Value::String(summary));
            store
        })
    });

    // Reducer: Concatenate all summaries into a single string
    let reducer = create_batch_node(|stores: Vec<SharedStore>| {
        Box::pin(async move {
            let mut all_summaries = Vec::new();
            for s in &stores {
                let summary = {
                    let guard = s.write().await;
                    guard
                        .get(SharedStoreKey::Summary.to_string().as_str())
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                };
                if let Some(summary) = summary {
                    all_summaries.push(summary);
                }
            }
            let mut result = HashMap::new();
            result.insert(
                SharedStoreKey::AllSummaries.to_string(),
                Value::String(all_summaries.join("\n")),
            );
            Arc::new(tokio::sync::RwLock::new(result))
        })
    });

    // Compose MapReduce
    let batch_mapper = Batch::new(mapper);
    let map_reduce = MapReduce::new(batch_mapper, reducer);

    // Run MapReduce
    let result = map_reduce.run(inputs).await;
    let result_map = {
        let guard = result.write().await;
        guard.clone()
    };

    println!(
        "All Summaries:\n{}",
        result_map
            .get(SharedStoreKey::AllSummaries.to_string().as_str())
            .unwrap()
    );
}
