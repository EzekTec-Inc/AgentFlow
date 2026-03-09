//! # Error Handling Example
//!
//! Demonstrates `ResultNode` / `create_result_node` for type-safe error propagation,
//! and `Agent::decide_result` for smart retry that distinguishes transient `Timeout`
//! errors from fatal errors.
//!
//! Run with:
//!   cargo run --example error-handling

use agentflow::{
    core::error::AgentFlowError,
    core::node::{create_node, create_result_node, SharedStore},
    patterns::agent::Agent,
    Flow,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{fmt, EnvFilter};

fn make_store(data: HashMap<String, serde_json::Value>) -> SharedStore {
    Arc::new(RwLock::new(data))
}

#[tokio::main]
async fn main() {
    // Initialise tracing so debug/warn lines from AgentFlow are visible.
    fmt()
        .with_env_filter(EnvFilter::new("agentflow=debug,error_handling=debug"))
        .init();

    println!("=== 1. ResultNode: success path ===");
    {
        let ok_node = create_result_node(|store: SharedStore| async move {
            store.write().await.insert(
                "result".to_string(),
                serde_json::Value::String("computed!".to_string()),
            );
            Ok(store)
        });

        let store = make_store(HashMap::new());
        match ok_node.call(store).await {
            Ok(s) => {
                let guard = s.read().await;
                println!("  result = {:?}", guard.get("result"));
            }
            Err(e) => println!("  unexpected error: {e}"),
        }
    }

    println!("\n=== 2. ResultNode: fatal NodeFailure ===");
    {
        let fail_node = create_result_node(|_store: SharedStore| async move {
            Err::<SharedStore, _>(AgentFlowError::NodeFailure(
                "database unavailable".to_string(),
            ))
        });

        let store = make_store(HashMap::new());
        match fail_node.call(store).await {
            Ok(_) => println!("  unexpected success"),
            Err(AgentFlowError::NodeFailure(msg)) => println!("  caught NodeFailure: {msg}"),
            Err(e) => println!("  other error: {e}"),
        }
    }

    println!("\n=== 3. Agent::decide_result — Timeout retried, then fails ===");
    {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc as StdArc;

        let call_count = StdArc::new(AtomicUsize::new(0));
        let cc = call_count.clone();

        let timeout_node = create_result_node(move |_store: SharedStore| {
            let cc = cc.clone();
            async move {
                let n = cc.fetch_add(1, Ordering::SeqCst) + 1;
                println!("    node call #{n} → Timeout");
                Err::<SharedStore, _>(AgentFlowError::Timeout(
                    "LLM response timed out".to_string(),
                ))
            }
        });

        // Agent<N> only needs N for decide_shared; decide_result takes an independent node ref.
        let agent = Agent::with_retry(create_node(|s: SharedStore| async { s }), 3, 0);

        let store = make_store(HashMap::new());
        match agent.decide_result(store, &timeout_node).await {
            Ok(_) => println!("  unexpected success"),
            Err(AgentFlowError::Timeout(msg)) => {
                println!("  exhausted retries; last error = Timeout({msg})");
                println!("  total calls = {}", call_count.load(Ordering::SeqCst));
            }
            Err(e) => println!("  other: {e}"),
        }
    }

    println!("\n=== 4. Agent::decide_result — fatal error aborts immediately ===");
    {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc as StdArc;

        let call_count = StdArc::new(AtomicUsize::new(0));
        let cc = call_count.clone();

        let fatal_node = create_result_node(move |_store: SharedStore| {
            let cc = cc.clone();
            async move {
                let n = cc.fetch_add(1, Ordering::SeqCst) + 1;
                println!("    node call #{n} → NodeFailure (fatal)");
                Err::<SharedStore, _>(AgentFlowError::NodeFailure("schema mismatch".to_string()))
            }
        });

        let agent = Agent::with_retry(create_node(|s: SharedStore| async { s }), 3, 0);

        let store = make_store(HashMap::new());
        match agent.decide_result(store, &fatal_node).await {
            Ok(_) => println!("  unexpected success"),
            Err(AgentFlowError::NodeFailure(msg)) => {
                println!("  aborted immediately with NodeFailure({msg})");
                println!("  total calls = {}", call_count.load(Ordering::SeqCst));
            }
            Err(e) => println!("  other: {e}"),
        }
    }

    println!("\n=== 5. Flow::run_safe — ExecutionLimitExceeded ===");
    {
        let mut flow = Flow::new().with_max_steps(3);

        let node_a = create_node(|store: SharedStore| async move {
            store.write().await.insert(
                "action".to_string(),
                serde_json::Value::String("loop".to_string()),
            );
            store
        });
        let node_b = create_node(|store: SharedStore| async move {
            store.write().await.insert(
                "action".to_string(),
                serde_json::Value::String("loop".to_string()),
            );
            store
        });

        flow.add_node("A", node_a);
        flow.add_node("B", node_b);
        flow.add_edge("A", "loop", "B");
        flow.add_edge("B", "loop", "A");

        let store = make_store(HashMap::new());
        match flow.run_safe(store).await {
            Ok(_) => println!("  unexpected success"),
            Err(AgentFlowError::ExecutionLimitExceeded(msg)) => {
                println!("  caught ExecutionLimitExceeded: {msg}");
            }
            Err(e) => println!("  other: {e}"),
        }
    }

    println!("\nDone.");
}
