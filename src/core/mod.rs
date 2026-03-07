//! Core abstractions: Node, Flow, SharedStore, Batch.

pub mod batch;
pub mod flow;
pub mod node;
pub mod store;

pub use batch::{Batch, ParallelBatch};
pub use flow::Flow;
pub use node::{
    create_batch_node, create_node, create_result_node, Node, NodeResult, ResultNode, SharedStore,
    SimpleNode,
};
pub use store::Store;
