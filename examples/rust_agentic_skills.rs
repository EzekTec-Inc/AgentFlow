use agentflow::core::flow::Flow;
use agentflow::core::node::{create_node, SharedStore};
use agentflow::patterns::rpi::RpiWorkflow;
use agentflow::skills::Skill;
use agentflow::utils::tool::create_tool_node;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// A simulated LLM that handles Research, Plan, Implement, and Verify phases.
/// In a real application, you would connect this to Rig, OpenAI, etc.
async fn simulated_llm(system_prompt: &str, user_prompt: &str) -> String {
    println!("  [LLM Called] System: {}", system_prompt);
    println!("  [LLM Called] User: {}", user_prompt);

    if system_prompt.contains("Research") {
        "Research complete: Cargo requires 'cargo init' to create a new project. We can use the shell tool to run it.".to_string()
    } else if system_prompt.contains("Plan") {
        "Plan: 1. Run 'cargo init my_new_project' using the shell tool. 2. Verify creation."
            .to_string()
    } else if system_prompt.contains("Implement") {
        "Implementation output: Command planned. I will execute tool_node.".to_string()
    } else if system_prompt.contains("Verify") {
        "Verification: The directory was created successfully.".to_string()
    } else {
        "Default response".to_string()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Rust Agentic Skills Example ===");

    // 1. Create a dummy SKILL.md for the example
    let skill_content = r#"---
name: Project Generator
description: Creates a new Rust project
version: 1.0.0
---
You are an expert Rust developer. Follow the RPI workflow to create a new Cargo project.
"#;
    let skill = Skill::parse(skill_content)?;
    println!(
        "Loaded Skill: {} (v{})",
        skill.name,
        skill.version.unwrap_or_default()
    );
    println!("Description: {}", skill.description);

    // 2. Setup the store
    let store: SharedStore = Arc::new(Mutex::new(HashMap::new()));

    // Inject the initial prompt
    {
        let mut guard = store.lock().await;
        guard.insert(
            "user_prompt".to_string(),
            Value::String("Create a new Rust project called my_new_project".to_string()),
        );
    }

    // 3. Create RPI workflow nodes using our simulated LLM
    let research_node = create_node({
        let skill_instructions = skill.instructions.clone();
        move |s| {
            let skill_instructions = skill_instructions.clone();
            Box::pin(async move {
                let prompt = {
                    let guard = s.lock().await;
                    guard
                        .get("user_prompt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                };

                let sys = format!("{}\nRole: Research", skill_instructions);
                let result = simulated_llm(&sys, &prompt).await;

                s.lock()
                    .await
                    .insert("research_output".to_string(), Value::String(result));
                s
            })
        }
    });

    let plan_node = create_node({
        let skill_instructions = skill.instructions.clone();
        move |s| {
            let skill_instructions = skill_instructions.clone();
            Box::pin(async move {
                let research = {
                    let guard = s.lock().await;
                    guard
                        .get("research_output")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                };

                let sys = format!("{}\nRole: Plan", skill_instructions);
                let result = simulated_llm(&sys, &research).await;

                s.lock()
                    .await
                    .insert("plan_output".to_string(), Value::String(result));
                s
            })
        }
    });

    let implement_node = create_node({
        let skill_instructions = skill.instructions.clone();
        move |s| {
            let skill_instructions = skill_instructions.clone();
            Box::pin(async move {
                let plan = {
                    let guard = s.lock().await;
                    guard
                        .get("plan_output")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                };

                let sys = format!("{}\nRole: Implement", skill_instructions);
                let result = simulated_llm(&sys, &plan).await;

                s.lock()
                    .await
                    .insert("implement_output".to_string(), Value::String(result));
                s
            })
        }
    });

    let verify_node = create_node({
        let skill_instructions = skill.instructions.clone();
        move |s| {
            let skill_instructions = skill_instructions.clone();
            Box::pin(async move {
                let impl_out = {
                    let guard = s.lock().await;
                    guard
                        .get("implement_output")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                };

                let sys = format!("{}\nRole: Verify", skill_instructions);
                let result = simulated_llm(&sys, &impl_out).await;

                s.lock()
                    .await
                    .insert("verify_output".to_string(), Value::String(result));
                s
            })
        }
    });

    // 4. Create an external tool node
    // Let's run a harmless command like "echo" to demonstrate the tool execution
    let tool_node = create_tool_node(
        "echo_tool",
        "echo",
        vec!["Running my_new_project generation steps...".to_string()],
    );

    // 5. Build the graph
    let rpi_workflow = RpiWorkflow::new()
        .with_research(research_node)
        .with_plan(plan_node)
        .with_implement(implement_node)
        .with_verify(verify_node);

    // 6. Execute the workflow
    println!("\nExecuting RPI Workflow...");
    let store_after_rpi: SharedStore = rpi_workflow.run(store).await;

    println!("\nExecuting Tool Workflow...");
    let mut tool_flow = Flow::new();
    tool_flow.add_node("tool", tool_node);
    let final_store: SharedStore = tool_flow.run(store_after_rpi).await;

    // 7. Review the results
    println!("\n=== Final Store Output ===");
    let guard = final_store.lock().await;

    println!(
        "Research Output: {}",
        guard.get("research_output").unwrap().as_str().unwrap()
    );
    println!(
        "Plan Output: {}",
        guard.get("plan_output").unwrap().as_str().unwrap()
    );
    println!(
        "Implement Output: {}",
        guard.get("implement_output").unwrap().as_str().unwrap()
    );
    println!(
        "Verify Output: {}",
        guard.get("verify_output").unwrap().as_str().unwrap()
    );

    // Check tool output
    if let Some(stdout) = guard.get("echo_tool_stdout") {
        println!("Tool Output (stdout): {}", stdout.as_str().unwrap().trim());
    }
    if let Some(status) = guard.get("echo_tool_status") {
        println!("Tool Exit Status: {}", status.as_i64().unwrap());
    }

    Ok(())
}
