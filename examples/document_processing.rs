use agentflow::prelude::*;
use agentflow::skills::Skill;
use agentflow::utils::tool::create_tool_node;
use serde_json::Value;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut workflow = Workflow::new();

    // 1. Classifier Node: Route based on document type (image vs text)
    workflow.add_step(
        "classify",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let file_path = {
                    let guard = store.write().await;
                    guard
                        .get("input_file")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                };

                let is_image = file_path.ends_with(".png") || file_path.ends_with(".jpg");

                {
                    let mut guard = store.write().await;
                    guard.insert(
                        "doc_type".to_string(),
                        Value::String(if is_image {
                            "image".into()
                        } else {
                            "text".into()
                        }),
                    );

                    if is_image {
                        guard.insert("action".to_string(), Value::String("extract_image".into()));
                    } else {
                        guard.insert("action".to_string(), Value::String("extract_text".into()));
                    }
                }
                store
            })
        }),
    );

    // 2a. Image Extraction Node (Mocked OCR)
    workflow.add_step(
        "extract_image",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                println!("[Workflow] Extracting from image...");
                {
                    let mut guard = store.write().await;
                    guard.insert(
                        "extracted_entities".to_string(),
                        Value::String("Mocked OCR text containing entities".into()),
                    );
                    guard.insert(
                        "action".to_string(),
                        Value::String("analyze_semantics".into()),
                    );
                }
                store
            })
        }),
    );

    // 2b. Text Extraction Node (LLM-based entity extraction)
    workflow.add_step(
        "extract_text",
        create_node(move |store: SharedStore| {
            Box::pin(async move {
                println!("[Workflow] Extracting entities from text using LLM...");

                // Mocking file read content for demonstration
                let _content = "The contract was signed by John Doe on behalf of Acme Corp for 5000 USD on 2024-01-15.";

                // Mocking LLM response to run example without OPENAI_API_KEY
                let response = "- Person: John Doe\n- Company: Acme Corp\n- Amount: 5000 USD\n- Date: 2024-01-15".to_string();

                let mut guard = store.write().await;
                guard.insert("extracted_entities".to_string(), Value::String(response));
                guard.insert("action".to_string(), Value::String("analyze_semantics".into()));
                drop(guard);

                store
            })
        }),
    );

    // Retry Loop Node (Intelligence: Retry extraction up to 3 times on failure)
    workflow.add_step(
        "retry_extract",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                let retries = {
                    let guard = store.write().await;
                    guard.get("retries").and_then(|v| v.as_i64()).unwrap_or(0)
                };

                {
                    let mut guard = store.write().await;
                    if retries < 3 {
                        println!(
                            "[Workflow] Retrying extraction (Attempt {})...",
                            retries + 1
                        );
                        guard.insert("retries".to_string(), Value::Number((retries + 1).into()));
                        guard.insert("action".to_string(), Value::String("extract_text".into()));
                    } else {
                        println!("[Workflow] Max retries reached. Failing.");
                        guard.insert("action".to_string(), Value::String("fail".into()));
                    }
                }
                store
            })
        }),
    );

    // 3. LLM Assessment Node (Intelligence: Assess extraction success & semantics)
    workflow.add_step(
        "analyze_semantics",
        create_node(move |store: SharedStore| {
            Box::pin(async move {
                println!("[Workflow] Analyzing semantics and assessing extraction quality...");

                let _entities = {
                    let guard = store.write().await;
                    guard
                        .get("extracted_entities")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                };

                // Mocking LLM response to run example without OPENAI_API_KEY
                let response = "SUCCESS: Context is a business contract signing.".to_string();
                let response_result: Result<String, String> = Ok(response);

                match response_result {
                    Ok(response) => {
                        let mut guard = store.write().await;
                        if response.contains("SUCCESS") {
                            guard.insert("semantics".to_string(), Value::String(response.clone()));
                            let doc_type = guard.get("doc_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            if doc_type == "image" {
                                guard.insert("action".to_string(), Value::String("convert_image".into()));
                            } else {
                                guard.insert("action".to_string(), Value::String("convert_text".into()));
                            }
                        } else {
                            println!("[Workflow] LLM Assessment deemed extraction inadequate. Routing to retry.");
                            guard.insert("action".to_string(), Value::String("retry_extract".into()));
                        }
                    }
                    Err(_) => {
                        let mut guard = store.write().await;
                        guard.insert("action".to_string(), Value::String("fail".into()));
                    }
                }
                store
            })
        }),
    );

    // 4. Fail Node
    workflow.add_step(
        "fail",
        create_node(|store: SharedStore| {
            Box::pin(async move {
                println!("[Workflow] Processing Failed.");
                {
                    let mut guard = store.write().await;
                    guard.insert("action".to_string(), Value::String("default".into()));
                    // End
                }
                store
            })
        }),
    );

    // Parse skills from the standard rust-agentic-skills format
    let skill = Skill::from_file("examples/SKILL_DOC_PROCESS.md").await?;

    // Dynamically register tools from the SKILL_DOC_PROCESS.md file
    if let Some(tools) = skill.tools {
        for tool in tools {
            // In a real application, we would use the parsed `command` and `args`
            // For the sake of this example, we mock the execution with bash to avoid 
            // failures if `pandoc` or `imagemagick` are not installed on the host machine.
            let mock_script = format!("echo 'Executing {}... Mock tool output.'", tool.command);
            let tool_node = create_tool_node(
                &tool.name,
                "bash",
                vec!["-c".into(), mock_script],
            );
            workflow.add_step(&tool.name, tool_node);
        }
    }

    // End node
    workflow.add_step("end", create_node(|store| Box::pin(async move { store })));

    // Wire up the workflow graph
    workflow.connect("classify", "extract_image"); // if action == "extract_image"
    workflow.connect("classify", "extract_text"); // if action == "extract_text"

    workflow.connect("extract_image", "analyze_semantics");

    workflow.connect("extract_text", "analyze_semantics");
    workflow.connect("extract_text", "retry_extract");

    workflow.connect("retry_extract", "extract_text");
    workflow.connect("retry_extract", "fail");

    workflow.connect("analyze_semantics", "convert_image");
    workflow.connect("analyze_semantics", "convert_text");
    workflow.connect("analyze_semantics", "retry_extract");
    workflow.connect("analyze_semantics", "fail");

    workflow.connect("convert_text", "end"); // By default goes to end
    workflow.connect("convert_image", "end");

    // Execute the workflow
    let mut store = HashMap::new();
    store.insert(
        "input_file".to_string(),
        Value::String("sample_document.txt".into()),
    );
    store.insert(
        "output_file".to_string(),
        Value::String("sample_document.pdf".into()),
    );

    println!("=== Starting Document Processing Workflow ===");
    let result = workflow.execute(store).await;

    // Display final results
    println!("\n=== Final Store Output ===");
    println!(
        "Document Type: {:?}",
        result
            .get("doc_type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    );
    println!(
        "Extracted Entities:\n{}",
        result
            .get("extracted_entities")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    );
    println!(
        "Context Semantics:\n{}",
        result
            .get("semantics")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    );
    if result.contains_key("convert_text_stdout") {
        println!(
            "Tool Output: {}",
            result
                .get("convert_text_stdout")
                .and_then(|v| v.as_str())
                .unwrap_or("")
        );
    }
    if result.contains_key("convert_image_stdout") {
        println!(
            "Tool Output: {}",
            result
                .get("convert_image_stdout")
                .and_then(|v| v.as_str())
                .unwrap_or("")
        );
    }

    Ok(())
}
