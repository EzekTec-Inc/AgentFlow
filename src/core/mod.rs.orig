//! Core abstractions: Node, Flow, SharedStore, Batch, ParallelFlow.

/// Batch execution primitives.
pub mod batch;
/// AgentFlow unified error types.
pub mod error;
/// Graph-based flow orchestrator.
pub mod flow;
/// Core node traits and types.
pub mod node;
pub mod parallel;
/// Shared state storage.
pub mod store;
/// Strongly-typed flow orchestrator.
pub mod typed_flow;
/// Strongly-typed state storage.
pub mod typed_store;

pub use batch::{Batch, ParallelBatch};
pub use error::AgentFlowError;
pub use flow::Flow;
pub use node::{
    create_batch_node, create_diff_node, create_node, create_result_node, Node, NodeResult,
    ResultNode, SharedStore, SimpleNode, StateDiff,
};
pub use parallel::ParallelFlow;
pub use store::Store;
pub use typed_flow::{create_typed_node, SimpleTypedNode, TypedFlow, TypedNode};
pub use typed_store::TypedStore;
