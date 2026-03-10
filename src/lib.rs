//! # AgentFlow
//!
//! A provider-agnostic, async-native Rust framework for building LLM agents,
//! RAG pipelines, multi-agent workflows, and agentic orchestration systems.
//!
//! ## Design philosophy
//!
//! - **Bring your own LLM** — AgentFlow handles orchestration; you supply the
//!   LLM calls inside nodes (use `rig-core`, `async-openai`, or any HTTP client).
//! - **Graph + Shared Store** — every pattern is built on a directed graph of
//!   [`Node`]s communicating through a [`SharedStore`].
//! - **Composable** — primitives snap together: a [`Flow`] can contain a
//!   [`Workflow`], a [`MultiAgent`] can contain [`Agent`]s, etc.
//! - **Async-first** — all execution is `async`/`await`; concurrent patterns
//!   use Tokio tasks under the hood.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use agentflow::prelude::*;
//! use std::collections::HashMap;
//! use std::sync::Arc;
//! use tokio::sync::RwLock;
//!
//! #[tokio::main]
//! async fn main() {
//!     let node_a = create_node(|store: SharedStore| async move {
//!         store.write().await.insert("msg".into(), serde_json::json!("hello from A"));
//!         store.write().await.insert("action".into(), serde_json::json!("next"));
//!         store
//!     });
//!
//!     let node_b = create_node(|store: SharedStore| async move {
//!         let msg = store.read().await.get("msg")
//!             .and_then(|v| v.as_str()).unwrap_or("").to_string();
//!         store.write().await.insert("final".into(), serde_json::json!(format!("B got: {msg}")));
//!         store
//!     });
//!
//!     let mut flow = Flow::new();
//!     flow.add_node("a", node_a);
//!     flow.add_node("b", node_b);
//!     flow.add_edge("a", "next", "b");
//!
//!     let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
//!     let result = flow.run(store).await;
//!     println!("{}", result.read().await["final"]); // B got: hello from A
//! }
//! ```
//!
//! ## Feature flags
//!
//! | Flag | Enables |
//! |------|---------|
//! | `skills` | YAML skill parser, tool nodes, `RpiWorkflow` |
//! | `mcp` | MCP stdio server (implies `skills`) |
//! | `rag` | Qdrant-backed retrieval |
//! | `repl` | Interactive REPL / TUI via `inquire` |
//!
//! ## Crate layout
//!
//! - [`core`] — [`Node`], [`Flow`], [`SharedStore`], [`Store`], [`crate::core::typed_store::TypedStore`],
//!   [`crate::core::typed_flow::TypedFlow`], [`Batch`], [`crate::core::error::AgentFlowError`]
//! - [`patterns`] — [`Agent`], [`Workflow`], [`MultiAgent`], [`Rag`],
//!   [`MapReduce`], [`StructuredOutput`]
//! - [`utils`] — shell tool nodes
//! - `skills` *(feature)* — skill file parser, `RpiWorkflow`
//! - `mcp` *(feature)* — MCP server

pub mod core;
pub mod patterns;
pub mod utils;

#[cfg(feature = "skills")]
pub mod skills;

#[cfg(feature = "mcp")]
pub mod mcp;

/// Convenience re-exports — import everything you need with `use agentflow::prelude::*`.
pub mod prelude {
    pub use crate::core::batch::{Batch, ParallelBatch};
    pub use crate::core::error::AgentFlowError;
    pub use crate::core::flow::Flow;
    pub use crate::core::node::{
        create_batch_node, create_node, create_result_node, Node, NodeResult, ResultNode,
        SharedStore, SimpleNode,
    };
    pub use crate::core::store::Store;
    pub use crate::patterns::agent::Agent;
    pub use crate::patterns::mapreduce::MapReduce;
    pub use crate::patterns::multi_agent::{MergeStrategy, MultiAgent};
    pub use crate::patterns::rag::Rag;
    pub use crate::patterns::structured_output::StructuredOutput;
    pub use crate::patterns::workflow::Workflow;
}

// Direct exports to match a flat namespace
pub use crate::core::batch::{Batch, ParallelBatch};
pub use crate::core::flow::Flow;
pub use crate::core::node::{
    create_batch_node, create_node, create_result_node, Node, NodeResult, ResultNode, SharedStore,
    SimpleNode,
};
pub use crate::core::store::Store;
pub use crate::patterns::agent::Agent;
pub use crate::patterns::mapreduce::MapReduce;
pub use crate::patterns::multi_agent::{MergeStrategy, MultiAgent};
pub use crate::patterns::rag::Rag;
pub use crate::patterns::structured_output::StructuredOutput;
pub use crate::patterns::workflow::Workflow;
