/*!
# Example: document_processing.rs

Real-world document processing pipeline. The workflow:

1. **Classify** — detects document type (image vs text) from the file extension
2. **Extract** — LLM extracts named entities from the document content
3. **Analyze** — LLM assesses extraction quality and determines semantic context
4. **Retry** — re-runs extraction up to 3 times if the LLM deems quality poor
5. **Convert** — runs a real shell tool (`pandoc` for text, `convert` for images)
   loaded dynamically from `SKILL_DOC_PROCESS.md`
6. **End** — prints a summary

Domain: contract / business document processing.

Requires: OPENAI_API_KEY
Optional: pandoc, imagemagick (falls back to echo mock if not installed)
Run with: cargo run --example document-processing
*/

use agentflow::prelude::*;
use agentflow::skills::Skill;
use agentflow::utils::tool::create_tool_node;
use dotenvy::dotenv;
use rig::prelude::*;
use rig::{completion::Prompt, providers};
use serde_json::Value;
use std::collections::HashMap;
use tracing_subscriber::{fmt, EnvFilter};

async fn llm(system: &str, user: &str) -> String {
    let client = providers::openai::Client::from_env();
    let agent = client.agent("gpt-4.1-mini").preamble(system).build();
    match agent.prompt(user).await {
        Ok(r) => r,
        Err(e) => format!("LLM error: {e}"),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    fmt()
        .with_env_filter(EnvFilter::new("agentflow=debug,document_processing=debug"))
        .init();

    let mut workflow = Workflow::new();

    // ── 1. Classify: detect document type from extension ─────────────────────
    workflow.add_step(
        "classify",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let file_path = {
                    let g = store.read().await;
                    g.get("input_file")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                };
                let is_image = file_path.ends_with(".png")
                    || file_path.ends_with(".jpg")
                    || file_path.ends_with(".jpeg");

                let (doc_type, action) = if is_image {
                    ("image", "extract_image")
                } else {
                    ("text", "extract_text")
                };

                println!("[Classify] '{}' → type={}", file_path, doc_type);
                //NOTE: the write lock is acquired here.
                let mut g = store.write().await;
                g.insert("doc_type".to_string(), Value::String(doc_type.to_string()));
                g.insert("action".to_string(), Value::String(action.to_string()));
                //NOTE: dropping the guard to the write lock is important.
                drop(g);

                store
            })
        }),
    );

    // ── 2a. Extract image — LLM-based OCR description ────────────────────────
    workflow.add_step(
        "extract_image",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let file_path = {
                    let g = store.read().await;
                    g.get("input_file")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                };
                println!("[ExtractImage] Describing image: {}", file_path);

                // Prompt the LLM to simulate what an OCR + vision pipeline would produce.
                // In production swap this for a vision-capable model call with the actual image.
                let entities = llm(
                    "You are an OCR and entity extraction engine. Given an image file path, \
                     produce a plausible list of entities (names, organisations, dates, amounts) \
                     that might appear in a business document with that name. \
                     Format: bullet list, e.g. '- Person: ...'",
                    &format!("Image file: {}", file_path),
                )
                .await;

                println!("[ExtractImage] Entities:\n{}", entities.trim());
                let mut g = store.write().await;
                g.insert("extracted_entities".to_string(), Value::String(entities));
                g.insert("retries".to_string(), Value::Number(0.into()));
                g.insert(
                    "action".to_string(),
                    Value::String("analyze_semantics".to_string()),
                );
                drop(g);
                store
            })
        }),
    );

    // ── 2b. Extract text — LLM entity extraction from real file content ───────
    workflow.add_step(
        "extract_text",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let (file_path, retries) = {
                    let g = store.read().await;
                    (
                        g.get("input_file")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        g.get("retries").and_then(|v| v.as_u64()).unwrap_or(0),
                    )
                };

                // Read the file, or fall back to a sample if it doesn't exist
                let content = tokio::fs::read_to_string(&file_path)
                    .await
                    .unwrap_or_else(|_| {
                        "The contract was signed by Jane Smith on behalf of Globex Ltd \
                     for 12 500 USD on 2025-06-01. Witnessed by Alan Turing."
                            .to_string()
                    });

                println!(
                    "[ExtractText] Attempt {} — extracting entities…",
                    retries + 1
                );
                let entities = llm(
                    "You are a named-entity extraction engine for business documents. \
                     Extract all persons, organisations, dates, and monetary amounts. \
                     Format as a bullet list: '- <Type>: <Value>'. Output only the list.",
                    &content,
                )
                .await;

                println!("[ExtractText] Entities:\n{}", entities.trim());
                let mut g = store.write().await;
                g.insert("extracted_entities".to_string(), Value::String(entities));
                g.insert(
                    "action".to_string(),
                    Value::String("analyze_semantics".to_string()),
                );
                drop(g);
                store
            })
        }),
    );

    // ── 3. Retry loop ─────────────────────────────────────────────────────────
    workflow.add_step(
        "retry_extract",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let retries = {
                    let g = store.read().await;
                    g.get("retries").and_then(|v| v.as_u64()).unwrap_or(0)
                };
                let mut g = store.write().await;
                if retries < 3 {
                    println!("[Retry] Retry attempt {}…", retries + 1);
                    g.insert("retries".to_string(), Value::Number((retries + 1).into()));
                    g.insert(
                        "action".to_string(),
                        Value::String("extract_text".to_string()),
                    );
                } else {
                    println!("[Retry] Max retries reached — failing.");
                    g.insert("action".to_string(), Value::String("fail".to_string()));
                }
                drop(g);
                store
            })
        }),
    );

    // ── 4. Analyze semantics — LLM quality assessment + context classification ─
    workflow.add_step(
        "analyze_semantics",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let (entities, doc_type) = {
                    let g = store.read().await;
                    (
                        g.get("extracted_entities")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        g.get("doc_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("text")
                            .to_string(),
                    )
                };

                println!("[Analyze] Assessing extraction quality and context…");
                let assessment = llm(
                    "You are a document-quality assessor. Given extracted entities from a business \
                     document, decide if the extraction is complete and coherent. \
                     Respond with exactly one of:\n\
                       SUCCESS: <one-sentence description of document context>\n\
                       FAIL: <one-sentence reason extraction is inadequate>",
                    &format!("Extracted entities:\n{}", entities),
                ).await;

                println!("[Analyze] Assessment: {}", assessment.trim());

                let mut g = store.write().await;
                if assessment.trim().starts_with("SUCCESS") {
                    g.insert("semantics".to_string(), Value::String(assessment.clone()));
                    let next = if doc_type == "image" {
                        "convert_image"
                    } else {
                        "convert_text"
                    };
                    g.insert("action".to_string(), Value::String(next.to_string()));
                } else {
                    println!("[Analyze] Quality check failed — routing to retry.");
                    g.insert(
                        "action".to_string(),
                        Value::String("retry_extract".to_string()),
                    );
                }
                drop(g);
                store
            })
        }),
    );

    // ── 5. Fail terminal node ─────────────────────────────────────────────────
    workflow.add_step(
        "fail",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                println!("[Fail] Document processing failed after max retries.");
                store
                    .write()
                    .await
                    .insert("action".to_string(), Value::String("default".to_string()));
                store
            })
        }),
    );

    // ── 6. End node ───────────────────────────────────────────────────────────
    workflow.add_step("end", create_node(|store| Box::pin(async move { store })));

    // ── Load conversion tools from skill file ─────────────────────────────────
    let skill = Skill::from_file("examples/SKILL_DOC_PROCESS.md").await?;
    if let Some(tools) = skill.tools {
        for tool in tools {
            // Use the real command; fall back to echo if the binary isn't installed
            let available = std::process::Command::new("which")
                .arg(&tool.command)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            let tool_node = if available {
                // Substitute {{input_file}} / {{output_file}} placeholders at build time
                // (runtime substitution would require a wrapper node — kept simple here)
                create_tool_node(&tool.name, &tool.command, tool.args.clone())
            } else {
                println!(
                    "[Skill] '{}' not found — using echo mock for '{}'",
                    tool.command, tool.name
                );
                create_tool_node(
                    &tool.name,
                    "echo",
                    vec![format!("Mock: {} would run here.", tool.command)],
                )
            };
            workflow.add_step(&tool.name, tool_node);
        }
    }

    // ── Wire graph ────────────────────────────────────────────────────────────
    workflow.connect_with_action("classify", "extract_image", "extract_image");
    workflow.connect_with_action("classify", "extract_text", "extract_text");
    workflow.connect_with_action("extract_image", "analyze_semantics", "analyze_semantics");
    workflow.connect_with_action("extract_text", "analyze_semantics", "analyze_semantics");
    workflow.connect_with_action("analyze_semantics", "convert_text", "convert_text");
    workflow.connect_with_action("analyze_semantics", "convert_image", "convert_image");
    workflow.connect_with_action("analyze_semantics", "retry_extract", "retry_extract");
    workflow.connect_with_action("analyze_semantics", "fail", "fail");
    workflow.connect_with_action("retry_extract", "extract_text", "extract_text");
    workflow.connect_with_action("retry_extract", "fail", "fail");
    workflow.connect("convert_text", "end");
    workflow.connect("convert_image", "end");

    // ── Run ───────────────────────────────────────────────────────────────────
    let mut store = HashMap::new();
    store.insert(
        "input_file".to_string(),
        Value::String("sample_contract.txt".to_string()),
    );
    store.insert(
        "output_file".to_string(),
        Value::String("sample_contract.pdf".to_string()),
    );

    println!("=== Document Processing Workflow ===\n");
    let result = workflow.execute(store).await;

    println!("\n=== Results ===");
    println!(
        "Type:      {}",
        result
            .get("doc_type")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
    );
    println!(
        "Entities:\n{}",
        result
            .get("extracted_entities")
            .and_then(|v| v.as_str())
            .unwrap_or("(none)")
    );
    println!(
        "Semantics: {}",
        result
            .get("semantics")
            .and_then(|v| v.as_str())
            .unwrap_or("(none)")
    );
    for key in &["convert_text_stdout", "convert_image_stdout"] {
        if let Some(out) = result.get(*key).and_then(|v| v.as_str()) {
            println!("Tool output: {}", out.trim());
        }
    }

    Ok(())
}
