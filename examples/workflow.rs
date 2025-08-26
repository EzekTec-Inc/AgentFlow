/*!
# Example: workflow.rs

**Purpose:**
Demonstrates a real-world, multi-step workflow for a Land Registry Agency, with Human-in-the-Loop (HITL) at each step.

**How it works:**
- Each step is an LLM agent: title search, title issuance, legal review.
- After each step, the result is shown to the user, who can approve, request revision, restart, or cancel.
- The workflow advances or repeats based on user input.

**How to adapt:**
- Replace the steps with your own business process (e.g., document review, multi-stage approval).
- Use the HITL pattern to add user oversight to any workflow.

**Example:**
```rust
let mut workflow = Workflow::new();
workflow.add_step("step1", ...);
workflow.add_step("step2", ...);
workflow.connect("step1", "step2");
let result = workflow.execute(store).await;
```
*/

use agentflow::prelude::*;
use rig::prelude::*;
use rig::{completion::Prompt, providers};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

fn prompt_user(step: &str, result: &serde_json::Value) -> String {
    println!("\n--- Step: {} ---", step);
    println!("Result of last processing:\n{}\n", result);
    println!("Options: [a]pprove, [r]equest revision, [d]eny/restart, [c]ancel");
    print!("Your choice: ");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_lowercase()
}

#[tokio::main]
async fn main() {
    let applicant_name = "John Doe";
    let property_desc = "Plot 40, Maple Estate, Springfield";

    // Step 1: Title Search Agent (LLM)
    let title_search_node = create_node({
        let property_desc = property_desc.to_string();
        move |store: SharedStore| {
            Box::pin({
                let value = property_desc.clone();
                async move {
                    let prompt = format!(
                        "You are a land registry search officer. Perform a title search for the following property: '{}'. List any encumbrances, prior owners, and confirm if the title is clear for transfer.",
                        value
                    );
                    let client = providers::openai::Client::from_env();
                    let rig_agent = client.agent("gpt-4.1-mini")
                        .preamble("You are a diligent land registry search officer.")
                        .build();

                    let response = match rig_agent.prompt(&prompt).await {
                        Ok(resp) => resp,
                        Err(e) => format!("Error: {}", e),
                    };

                    store.lock().unwrap().insert("title_search".to_string(), Value::String(response));
                    store.lock().unwrap().insert("action".to_string(), Value::String("default".to_string()));
                    store
                }
            })
        }
    });

    // Step 2: Title Issuance Agent (LLM)
    let title_issuance_node = create_node({
        let applicant_name = applicant_name.to_string();
        let property_desc = property_desc.to_string();
        move |store: SharedStore| {
            Box::pin({
                let applicant_name = applicant_name.clone();
                let property_desc = property_desc.clone();
                async move {
                    let search_result = store
                        .lock()
                        .unwrap()
                        .get("title_search")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let prompt = format!(
                        "You are a land registry officer. Based on the following title search result:\n{}\n\nPrepare a draft land title issuance for applicant '{}', property '{}'. Include all relevant legal language and conditions.",
                        search_result, applicant_name, property_desc
                    );
                    let client = providers::openai::Client::from_env();
                    let rig_agent = client.agent("gpt-3.5-turbo")
                        .preamble("You are a land registry officer specializing in title issuance.")
                        .build();

                    let response = match rig_agent.prompt(&prompt).await {
                        Ok(resp) => resp,
                        Err(e) => format!("Error: {}", e),
                    };

                    store.lock().unwrap().insert("title_issuance".to_string(), Value::String(response));
                    store.lock().unwrap().insert("action".to_string(), Value::String("default".to_string()));
                    store
                }
            })
        }
    });

    // Step 3: Legal Review Agent (LLM)
    let legal_review_node = create_node(|store: SharedStore| {
        Box::pin(async move {
            let issuance = store
                .lock()
                .unwrap()
                .get("title_issuance")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let prompt = format!(
                "You are a legal officer. Review the following draft land title issuance for legal sufficiency, compliance, and clarity. Suggest any corrections or improvements.\n\n{}",
                issuance
            );
            let client = providers::openai::Client::from_env();
            let rig_agent = client.agent("gpt-3.5-turbo")
                .preamble("You are a legal officer specializing in land title review.")
                .build();

            let response = match rig_agent.prompt(&prompt).await {
                Ok(resp) => resp,
                Err(e) => format!("Error: {}", e),
            };

            store.lock().unwrap().insert("legal_review".to_string(), Value::String(response));
            store.lock().unwrap().insert("action".to_string(), Value::String("default".to_string()));
            store
        })
    });

    // Build the workflow
    let mut workflow = Workflow::new();
    workflow.add_step("title_search", title_search_node);
    workflow.add_step("title_issuance", title_issuance_node);
    workflow.add_step("legal_review", legal_review_node);
    workflow.connect("title_search", "title_issuance");
    workflow.connect("title_issuance", "legal_review");

    let store = Arc::new(Mutex::new(HashMap::new()));
    let mut current_step = Some("title_search".to_string());
    let mut last_result = store.clone();

    while let Some(step) = current_step.clone() {
        // Run the current step
        let node = workflow.get_node(&step).unwrap();
        let result = node.call(last_result.clone()).await;

        // Present result to user and get action
        let locked = result.lock().unwrap();
        let step_result = locked.get(&step).cloned().unwrap_or(serde_json::Value::Null);
        drop(locked);

        // Show the result of the last processing before HITL interaction
        let user_action = prompt_user(&step, &step_result);

        let mut locked = result.lock().unwrap();
        match user_action.as_str() {
            "a" | "approve" => {
                locked.insert("action".to_string(), Value::String("default".to_string()));
                // Move to next step
                current_step = workflow.get_next_step(&step, "default");
            }
            "r" | "request revision" => {
                locked.insert("action".to_string(), Value::String("revise".to_string()));
                // Rerun the same step, possibly after user edits input (not shown here)
            }
            "d" | "deny" | "restart" => {
                locked.insert("action".to_string(), Value::String("default".to_string()));
                // Rerun the same step with the same input
            }
            "c" | "cancel" => {
                println!("Workflow cancelled. Last result:");
                println!("{:?}", step_result);
                return;
            }
            _ => {
                println!("Invalid input, assuming approve.");
                locked.insert("action".to_string(), Value::String("default".to_string()));
                current_step = workflow.get_next_step(&step, "default");
            }
        }
        drop(locked);
        last_result = result.clone();
    }

    println!("Workflow complete. Final result:");
    let locked = last_result.lock().unwrap();
    for (k, v) in locked.iter() {
        println!("{}: {}", k, v);
    }
}
