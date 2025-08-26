/*!
# Example: multi_agent.rs

**Purpose:**  
Demonstrates running multiple agents in parallel, each responsible for a different part of a software project (e.g., generating TypeScript, HTML, and TailwindCSS for a Space Invader game).

**How it works:**
- Each agent is an LLM node with a specialized prompt.
- All agents write their results to a shared store.
- A progress spinner is shown while agents work.
- Final results from all agents are displayed.

**How to adapt:**
- Use this pattern for any multi-role, multi-agent scenario (e.g., research, code, test, deploy).
- Add or remove agents as needed for your workflow.

**Example:**
```rust
let mut multi_agent = MultiAgent::new();
multi_agent.add_agent(agent1);
multi_agent.add_agent(agent2);
let result = multi_agent.run(store).await;
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
    // Shared project description
    let project_desc = "Create a fully-functional Space Invader game using TypeScript, HTML, and TailwindCSS. The game should be playable in a modern browser.";

    // Agent 1: TypeScript game logic (GPT-4.1-mini)
    let agent1 = create_node({
        let prompt = format!(
            "{desc}\n\nYour task: Write the complete TypeScript code for the game logic, including player movement, shooting, enemy behavior, collision detection, and game loop. Output only the TypeScript code.",
            desc = project_desc
        );
        move |store: SharedStore| {
            Box::pin({
                let value = prompt.clone();
                async move {
                    let openai_client = providers::openai::Client::from_env();
                    let rig_agent = openai_client
                        .agent("gpt-4.1-mini")
                        .preamble("You are a senior TypeScript game developer.")
                        .build();

                    let response = match rig_agent.prompt(&value).await {
                        Ok(resp) => resp,
                        Err(e) => format!("Error: {}", e),
                    };

                    store.lock().unwrap().insert(
                        "typescript".to_string(),
                        Value::String(response),
                    );
                    store
                }
            })
        }
    });

    // Agent 2: HTML structure (GPT-3.5-turbo)
    let agent2 = create_node({
        let prompt = format!(
            "{desc}\n\nYour task: Write the complete HTML structure for the game, including a canvas or game area, and any necessary UI elements. Use semantic HTML. Output only the HTML code.",
            desc = project_desc
        );
        move |store: SharedStore| {
            Box::pin({
                let value = prompt.clone();
                async move {
                    let openai_client = providers::openai::Client::from_env();
                    let rig_agent = openai_client
                        .agent("gpt-3.5-turbo")
                        .preamble("You are a senior frontend developer specializing in HTML.")
                        .build();

                    let response = match rig_agent.prompt(&value).await {
                        Ok(resp) => resp,
                        Err(e) => format!("Error: {}", e),
                    };

                    store.lock().unwrap().insert(
                        "html".to_string(),
                        Value::String(response),
                    );
                    store
                }
            })
        }
    });

    // Agent 3: TailwindCSS styles (Gemini)
    let agent3 = create_node({
        let prompt = format!(
            "{desc}\n\nYour task: Write the complete TailwindCSS classes and any custom styles needed for the game. Output only the relevant CSS or Tailwind class usage.",
            desc = project_desc
        );
        move |store: SharedStore| {
            Box::pin({
                let value = prompt.clone();
                async move {
                    let gemini_client = providers::gemini::Client::from_env();
                    let rig_agent = gemini_client
                        .agent("gemini-1.5-pro")
                        .preamble("You are a senior frontend developer specializing in TailwindCSS.")
                        .build();

                    let response = match rig_agent.prompt(&value).await {
                        Ok(resp) => resp,
                        Err(e) => format!("Error: {}", e),
                    };

                    store.lock().unwrap().insert(
                        "tailwindcss".to_string(),
                        Value::String(response),
                    );
                    store
                }
            })
        }
    });

    // Add all agents to MultiAgent
    let mut multi_agent = MultiAgent::new();
    multi_agent.add_agent(agent1);
    multi_agent.add_agent(agent2);
    multi_agent.add_agent(agent3);

    // Prepare the shared store
    let store = Arc::new(Mutex::new(HashMap::new()));

    // Show progress to the user while agents are working
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::Duration as StdDuration;

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // Spawn a progress thread
    let progress_handle = thread::spawn(move || {
        let spinner = ["|", "/", "-", "\\"];
        let mut i = 0;
        println!("Agents are working on your Space Invader game...");
        while running_clone.load(Ordering::SeqCst) {
            print!("\rProgress: {}", spinner[i % spinner.len()]);
            std::io::Write::flush(&mut std::io::stdout()).ok();
            thread::sleep(StdDuration::from_millis(200));
            i += 1;
        }
        println!("\rProgress: done!           ");
    });

    // Run all agents concurrently
    let result = multi_agent.run(store).await;

    // Stop the progress thread
    running.store(false, Ordering::SeqCst);
    progress_handle.join().ok();

    // Print the results from each agent
    let result_map = result.lock().unwrap();
    println!("=== Space Invader Game Artifacts ===\n");
    if let Some(ts) = result_map.get("typescript") {
        println!("--- TypeScript Game Logic ---\n{}\n", ts);
    }
    if let Some(html) = result_map.get("html") {
        println!("--- HTML Structure ---\n{}\n", html);
    }
    if let Some(css) = result_map.get("tailwindcss") {
        println!("--- TailwindCSS Styles ---\n{}\n", css);
    }
}
