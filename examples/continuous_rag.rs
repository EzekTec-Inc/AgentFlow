use agentflow::core::batch::ParallelBatch;
use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, Node, SharedStore};
use agentflow::patterns::rag::Rag;
use agentflow::patterns::skill::SkillInjector;
use agentflow::skills::{Skill, SkillTool};
use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::client::ProviderClient;
use rig::providers::openai::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    println!("Starting Continuous RAG Pipeline...");

    // Mock documents simulating a continuous ingestion stream
    let docs = vec![
        "AgentFlow is a framework for orchestrating LLM agents in Rust.",
        "AgentFlow uses Tokio for asynchronous execution and `SharedStore` for state.",
        "AgentFlow supports Human-in-the-Loop (HITL) workflows natively.",
        "ParallelBatch allows data parallelism for processing lists concurrently.",
    ];

    // 1. Data-Parallel Batching: Summarize Documents
    let summarizer = create_node(|store: SharedStore| {
        Box::pin(async move {
            let doc = store
                .read()
                .await
                .get("document")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let openai_client = Client::from_env();
            let agent = openai_client
                .agent("gpt-4o-mini")
                .preamble("Summarize in 10 words or less.")
                .build();

            let summary: String = agent
                .prompt(&doc)
                .await
                .unwrap_or_else(|e| format!("Error: {}", e));

            store
                .write()
                .await
                .insert("summary".to_string(), Value::String(summary));
            store
        })
    });

    let batch = ParallelBatch::new(summarizer);

    // Prepare inputs
    let mut inputs = Vec::new();
    for doc in docs {
        let store = Arc::new(RwLock::new(HashMap::new()));
        store
            .write()
            .await
            .insert("document".to_string(), Value::String(doc.to_string()));
        inputs.push(store);
    }

    println!("Batching {} documents in parallel...", inputs.len());
    let results = batch.call(inputs).await;

    // Simulated Vector DB Ingestion (using memory instead of Qdrant server for portability)
    let vector_db = Arc::new(RwLock::new(Vec::new()));
    for res in results {
        let summary = res
            .read()
            .await
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        vector_db.write().await.push(summary);
    }

    // 2. Query Pipeline (RAG Pattern with SkillInjector)

    // Create a mock Skill for querying the "Vector DB"
    let qdrant_skill = Skill {
        name: "query_qdrant".into(),
        version: "1.0.0".into(),
        description: "Queries the local Qdrant collection".into(),
        instructions: "Use this skill to fetch document summaries from the vector database. Format your response clearly as an expert.".into(),
        tools: vec![SkillTool {
            name: "search_db".into(),
            description: "Searches the vector DB".into(),
            command: "echo".into(),
            args: vec!["db_search_invoked".into()],
        }],
    };

    let skill_injector = SkillInjector::new(qdrant_skill);

    let retriever = create_node({
        let db = Arc::clone(&vector_db);
        move |store: SharedStore| {
            let db_clone = Arc::clone(&db);
            Box::pin(async move {
                // Mock retrieval: just join all summaries (simulating semantic search)
                let context = db_clone.read().await.join("\n");
                store
                    .write()
                    .await
                    .insert("context".to_string(), Value::String(context));
                store
            })
        }
    });

    let generator = create_node(|store: SharedStore| {
        Box::pin(async move {
            let context = store
                .read()
                .await
                .get("context")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let query = store
                .read()
                .await
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let skill_instructions = store
                .read()
                .await
                .get("skill_instructions")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let openai_client = Client::from_env();
            let preamble = format!(
                "You are a helpful assistant.\nSkill Instructions: {}",
                skill_instructions
            );
            let agent = openai_client.agent("gpt-4o-mini").preamble(&preamble).build();

            let answer: String = agent
                .prompt(&format!("Context: {}\n\nQuestion: {}", context, query))
                .await
                .unwrap_or_else(|e| format!("Error: {}", e));

            store
                .write()
                .await
                .insert("response".to_string(), Value::String(answer));
            store
        })
    });

    let rag = Rag::new(retriever, generator);

    // Combine them into a final Query flow
    let mut query_flow = Flow::new();
    query_flow.add_node("inject_skill", skill_injector);
    query_flow.add_node("rag_pipeline", rag);

    let query_store = Arc::new(RwLock::new(HashMap::new()));
    query_store
        .write()
        .await
        .insert("query".to_string(), Value::String("Does AgentFlow support Human-in-the-Loop?".to_string()));

    println!("Querying the ingested knowledge base...");
    let final_result = query_flow.run(query_store).await;

    let answer = final_result
        .read()
        .await
        .get("response")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    println!("\n=== Final Answer ===\n{}", answer);

    Ok(())
}