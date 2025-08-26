/*!
External Utility Examples
========================

We do NOT provide built-in utilities. Instead, we offer examples—please implement your own.

Why not built-in?
- API Volatility: Frequent changes lead to heavy maintenance for hardcoded APIs
- Flexibility: You may want to switch vendors, use fine-tuned models, or run them locally
- Optimizations: Prompt caching, batching, and streaming are easier without vendor lock-in

Implement these to integrate with external systems:

```mermaid
flowchart LR
    Node --> Utility
    Utility -->|API Call| ExternalSystem
```
*/

/// LLM Wrapper examples
pub mod llm {
    use crate::core::node::{create_node, Node, SharedStore};
    use serde_json::Value;

    /// Create a mock LLM node that simulates a response.
    pub fn create_mock_llm_node() -> Box<dyn Node<SharedStore, SharedStore>> {
        create_node(move |store: SharedStore| {
            Box::pin(async move {
                let prompt = store
                    .lock()
                    .unwrap()
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let response = format!("Mock response to: '{}'", prompt);
                store
                    .lock()
                    .unwrap()
                    .insert("response".to_string(), Value::String(response));
                store
            })
        })
    }
}

/// Web Search API wrapper example
pub mod web_search {
    use crate::core::node::{create_node, Node, SharedStore};
    use serde_json::Value;

    /// Example Google Search wrapper - implement your own
    pub fn create_google_search_node(
        api_key: String,
    ) -> Box<dyn Node<SharedStore, SharedStore>> {
        create_node(move |store: SharedStore| {
            let _api_key = api_key.clone();
            Box::pin(async move {
                let _query = store
                    .lock()
                    .unwrap()
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // TODO: Implement actual Google Search API call
                let results = vec![
                    "Example result 1".to_string(),
                    "Example result 2".to_string(),
                ];
                store.lock().unwrap().insert(
                    "search_results".to_string(),
                    Value::Array(results.into_iter().map(Value::String).collect()),
                );
                store
            })
        })
    }
}

/// Embedding examples
pub mod embedding {
    use crate::core::node::{create_node, Node, SharedStore};
    use serde_json::Value;

    /// Example embedding node - implement your own
    pub fn create_embedding_node() -> Box<dyn Node<SharedStore, SharedStore>> {
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let _text = store
                    .lock()
                    .unwrap()
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // TODO: Implement actual embedding generation
                let embedding = vec![0.1, 0.2, 0.3, 0.4, 0.5]; // Mock embedding
                store.lock().unwrap().insert(
                    "embedding".to_string(),
                    Value::Array(
                        embedding
                            .into_iter()
                            .map(|f| Value::Number(serde_json::Number::from_f64(f).unwrap()))
                            .collect(),
                    ),
                );
                store
            })
        })
    }
}

/// Vector Database examples
pub mod vector {
    use crate::core::node::{create_node, Node, SharedStore};
    use serde_json::Value;

    /// Example vector search node - implement your own
    pub fn create_vector_search_node() -> Box<dyn Node<SharedStore, SharedStore>> {
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let _query_embedding = store
                    .lock()
                    .unwrap()
                    .get("query_embedding")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                // TODO: Implement actual vector database search
                let similar_docs = vec![
                    "Similar document 1".to_string(),
                    "Similar document 2".to_string(),
                ];
                store.lock().unwrap().insert(
                    "similar_docs".to_string(),
                    Value::Array(similar_docs.into_iter().map(Value::String).collect()),
                );
                store
            })
        })
    }
}

/// Chunking examples
pub mod chunking {
    use crate::core::node::{create_node, Node, SharedStore};
    use serde_json::Value;

    /// Example text chunking node - implement your own
    pub fn create_chunking_node(chunk_size: usize) -> Box<dyn Node<SharedStore, SharedStore>> {
        create_node(move |store: SharedStore| {
            Box::pin(async move {
                let text = store
                    .lock()
                    .unwrap()
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // Simple chunking by character count
                let chunks: Vec<String> = text
                    .chars()
                    .collect::<Vec<_>>()
                    .chunks(chunk_size)
                    .map(|chunk| chunk.iter().collect())
                    .collect();

                store.lock().unwrap().insert(
                    "chunks".to_string(),
                    Value::Array(chunks.into_iter().map(Value::String).collect()),
                );
                store
            })
        })
    }
}
