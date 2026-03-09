//! Core abstractions: Node, Flow, SharedStore, Batch.

pub mod batch;
pub mod error;
pub mod flow;
pub mod node;
pub mod store;
pub mod typed_store;
pub mod typed_flow;

pub use batch::{Batch, ParallelBatch};
pub use error::AgentFlowError;
pub use flow::Flow;
pub use node::{
    create_batch_node, create_node, create_result_node, Node, NodeResult, ResultNode, SharedStore,
    SimpleNode,
};
pub use store::Store;
pub use typed_store::TypedStore;
pub use typed_flow::{TypedFlow, TypedNode, SimpleTypedNode, create_typed_node};
