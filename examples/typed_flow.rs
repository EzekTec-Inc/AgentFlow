/*!
# Example: typed_flow.rs

Real-world TypedFlow example: a multi-stage content pipeline backed by a real
LLM. The typed state carries a topic, a draft, a critique, and a revision
count. The flow loops through Draft → Critique → Revise until the LLM critic
approves or the revision limit is reached.

This showcases TypedFlow's key advantage over the HashMap-based Flow: the state
is a plain Rust struct — no string key lookups, full type safety.

Requires: OPENAI_API_KEY
Run with: cargo run --example typed-flow
*/

use agentflow::core::{create_typed_node, TypedFlow, TypedStore};
use dotenvy::dotenv;
use rig::prelude::*;
use rig::{completion::Prompt, providers};
use tracing_subscriber::{fmt, EnvFilter};

// ── Typed state ──────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
struct ContentState {
    topic: String,
    draft: String,
    critique: String,
    approved: bool,
    revisions: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Action {
    Review,
    Revise,
}

async fn llm(system: &str, user: &str) -> String {
    let client = providers::openai::Client::from_env();
    let agent = client.agent("gpt-4o-mini").preamble(system).build();
    match agent.prompt(user).await {
        Ok(r) => r,
        Err(e) => format!("LLM error: {e}"),
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    fmt()
        .with_env_filter(EnvFilter::new("agentflow=debug,typed_flow=debug"))
        .init();

    let mut flow = TypedFlow::<ContentState, Action>::new().with_max_steps(12);

    // ── Draft node ────────────���───────────────────────────────────────────────
    let draft_node = create_typed_node(|mut store: TypedStore<ContentState>| async move {
        let (topic, critique, revisions) = (store.inner.topic.clone(), store.inner.critique.clone(), store.inner.revisions);

        let prompt = if critique.is_empty() {
            format!(
                "Write a clear, engaging 3-sentence paragraph about: {}",
                topic
            )
        } else {
            format!(
                "Revise this paragraph about '{}' based on the feedback.\nFeedback: {}\nWrite only the revised paragraph.",
                topic, critique
            )
        };

        println!("\n[Draft] Revision {}…", revisions + 1);
        let draft = llm(
            "You are a skilled technical writer. Output only the paragraph — no preamble.",
            &prompt,
        )
        .await;
        println!("[Draft]\n{}\n", draft.trim());

        store.inner.draft = draft;
        store.inner.revisions += 1;
        (store, Some(Action::Review))
    });

    // ── Critique node ─────────────────────────────────────────────────────────
    let critique_node = create_typed_node(|mut store: TypedStore<ContentState>| async move {
        let draft = store.inner.draft.clone();

        println!("[Critique] Reviewing draft…");
        let verdict = llm(
            "You are a strict editor. Review the paragraph. \
             Respond with APPROVED or REVISE: <one-sentence reason>. No other text.",
            &draft,
        )
        .await;
        println!("[Critique] {}\n", verdict.trim());

        let mut approved = false;
        if verdict.trim().starts_with("APPROVED") {
            store.inner.approved = true;
            store.inner.critique = String::new();
            approved = true;
        } else {
            store.inner.approved = false;
            store.inner.critique = verdict
                .trim()
                .strip_prefix("REVISE:")
                .unwrap_or("")
                .trim()
                .to_string();
        }
        
        if approved {
            (store, None)
        } else {
            (store, Some(Action::Revise))
        }
    });

    flow.add_node("draft", draft_node);
    flow.add_node("critique", critique_node);

    flow.add_edge("draft", Action::Review, "critique");
    flow.add_edge("critique", Action::Revise, "draft");

    let topic = "How Rust's borrow checker prevents use-after-free bugs";

    let initial = ContentState {
        topic: topic.to_string(),
        draft: String::new(),
        critique: String::new(),
        approved: false,
        revisions: 0,
    };

    println!("=== TypedFlow Content Pipeline ===");
    println!("Topic: {}\n", topic);

    let store = TypedStore::new(initial);
    let final_store = flow.run(store).await;
    let final_state = final_store.inner;

    println!(
        "=== Approved Draft (after {} revision(s)) ===\n\n{}",
        final_state.revisions,
        final_state.draft.trim()
    );
}
