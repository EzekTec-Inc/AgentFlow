//! Core abstractions: Node, Flow, SharedStore, Batch.

pub mod node;
pub mod flow;
pub mod batch;

// Re-export core types for convenience
pub use node::{Node, SharedStore, SimpleNode, create_node, create_batch_node};
pub use flow::Flow;
pub use batch::{Batch, ParallelBatch};
