/*!
# Example: structured_output.rs

**Purpose:**
Implements a multi-agent, interactive TUI pipeline for research, summarization, and critique, with structured output and user-driven control.

**How it works:**
- The user enters a topic via a TUI menu.
- Three LLM agents run in sequence: research, summarize, critique.
- The output is structured as a JSON object and prettified for the user.
- The user can revise (re-run all agents) or cancel at any time.

**How to adapt:**
- Use this pattern for any multi-step, multi-agent pipeline where structured output and user control are important (e.g., report generation, content review, multi-stage analysis).
- Change the agent prompts and structuring logic to fit your domain.

**Example:**
```rust
let pipeline = StructuredOutput::new(
    create_node(move |store| { ... })
);
let result = pipeline.call(store).await;
```
*/

use agentflow::prelude::*;
use rig::prelude::*;
use rig::{completion::Prompt, providers};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::Utc;
use inquire::{Select, Text};

#[tokio::main]
async fn main() {
    let mut last_topic = String::new();
    let mut last_structured = None;

    loop {
        println!("\n=== Research & Critique TUI ===");
        let options = vec![
            "Enter new topic to process",
            "Revise last response (re-run all agents on last topic)",
            "Cancel and show prettified output",
        ];
        let choice = Select::new("Choose an action:", options.clone())
            .prompt()
            .unwrap_or_else(|_| options[2]);

        match choice {
            "Enter new topic to process" => {
                let topic = Text::new("Please enter a topic for research, summary, and critique:")
                    .prompt()
                    .unwrap_or_default();
                if topic.trim().is_empty() {
                    println!("No topic entered.");
                    continue;
                }
                last_topic = topic.clone();
                last_structured = process_topic(topic.clone()).await;
            }
            "Revise last response (re-run all agents on last topic)" => {
                if last_topic.is_empty() {
                    println!("No previous topic to revise. Please enter a new topic first.");
                    continue;
                }
                println!("Re-running all agents for last topic: '{}'", last_topic);
                last_structured = process_topic(last_topic.clone()).await;
            }
            "Cancel and show prettified output" => {
                println!("\n=== Final Structured Output ===");
                if let Some(structured) = &last_structured {
                    println!("{}", serde_json::to_string_pretty(structured).unwrap());
                } else {
                    println!("No structured output found.");
                }
                break;
            }
            _ => {
                println!("Invalid choice.");
            }
        }
    }
}

async fn process_topic(topic: String) -> Option<serde_json::Value> {
    println!("\nStep 1: User submitted topic:\n{}\n", topic);

    // Agent 1: Research agent (LLM)
    let research_node = create_node({
        let topic = topic.clone();
        move |store: SharedStore| {
            Box::pin({
                let value = topic.clone();
                async move {
                    println!("Agent 1: Researching topic with LLM...");
                    let prompt = format!(
                        "You are a research assistant. List 5 key facts or insights about the topic: '{}'.",
                        value
                    );
                    let client = providers::openai::Client::from_env();
                    let rig_agent = client.agent("gpt-4.1-mini")
                        .preamble("You are a research assistant.")
                        .build();

                    let research = match rig_agent.prompt(&prompt).await {
                        Ok(resp) => resp,
                        Err(e) => format!("Error: {}", e),
                    };

                    println!("Agent 1: Research output:\n{}\n", research);

                    store.lock().unwrap().insert("research".to_string(), Value::String(research));
                    store
                }
            })
        }
    });

    // Agent 2: Summarization agent (LLM)
    let summary_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            println!("Agent 2: Summarizing research with LLM...");
            let research = store
                .lock()
                .unwrap()
                .get("research")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let prompt = format!(
                "You are a summarization expert. Summarize the following research into a concise paragraph:\n{}",
                research
            );
            let client = providers::openai::Client::from_env();
            let rig_agent = client.agent("gpt-3.5-turbo")
                .preamble("You are a summarization expert.")
                .build();

            let summary = match rig_agent.prompt(&prompt).await {
                Ok(resp) => resp,
                Err(e) => format!("Error: {}", e),
            };

            println!("Agent 2: Summary output:\n{}\n", summary);

            store.lock().unwrap().insert("summary".to_string(), Value::String(summary));
            store
        })
    });

    // Agent 3: Critique agent (LLM)
    let critique_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            println!("Agent 3: Critiquing summary with LLM...");
            let summary = store
                .lock()
                .unwrap()
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let prompt = format!(
                "You are a critical reviewer. Critique the following summary for accuracy, clarity, and completeness. Suggest improvements if needed.\n{}",
                summary
            );
            let client = providers::openai::Client::from_env();
            let rig_agent = client.agent("gpt-4.1-mini")
                .preamble("You are a critical reviewer.")
                .build();

            let critique = match rig_agent.prompt(&prompt).await {
                Ok(resp) => resp,
                Err(e) => format!("Error: {}", e),
            };

            println!("Agent 3: Critique output:\n{}\n", critique);

            store.lock().unwrap().insert("critique".to_string(), Value::String(critique));
            store
        })
    });

    // Step 4: Structure the output
    let topic_clone = topic.clone();
    let structured_node = create_node(move |store: SharedStore| {
        let topic = topic_clone.clone();
        Box::pin(async move {
            println!("Step 4: Structuring output...");
            let research = store
                .lock()
                .unwrap()
                .get("research")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let summary = store
                .lock()
                .unwrap()
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let critique = store
                .lock()
                .unwrap()
                .get("critique")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let structured = serde_json::json!({
                "status": "success",
                "topic": topic,
                "research": research,
                "summary": summary,
                "critique": critique,
                "timestamp": Utc::now().to_rfc3339(),
            });
            store.lock().unwrap().insert("structured_output".to_string(), structured.clone());
            println!("Step 4: Output structured.\n");
            store
        })
    });

    // Compose the pipeline
    let pipeline = StructuredOutput::new(
        create_node(move |store: SharedStore| {
            let research_node = research_node.clone();
            let summary_node = summary_node.clone();
            let critique_node = critique_node.clone();
            let structured_node = structured_node.clone();
            Box::pin(async move {
                let store = research_node.call(store).await;
                let store = summary_node.call(store).await;
                let store = critique_node.call(store).await;
                structured_node.call(store).await
            })
        })
    );

    // Run the pipeline
    let mut store = HashMap::new();
    store.insert("topic".to_string(), Value::String(topic.to_string()));
    let result = pipeline.call(Arc::new(Mutex::new(store))).await;

    // Display the final structured output
    let locked = result.lock().unwrap();
    if let Some(structured) = locked.get("structured_output") {
        println!("=== Structured Output ===");
        println!("{}", serde_json::to_string_pretty(structured).unwrap());
        Some(structured.clone())
    } else {
        println!("No structured output found.");
        None
    }
}
