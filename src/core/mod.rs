//! Core abstractions: Node, Flow, SharedStore, Batch.

pub mod node;
pub mod flow;
pub mod batch;
pub mod store;

pub use node::{Node, NodeResult, SharedStore, SimpleNode, ResultNode, create_node, create_result_node, create_batch_node};
pub use flow::Flow;
pub use batch::{Batch, ParallelBatch};
pub use store::Store;
