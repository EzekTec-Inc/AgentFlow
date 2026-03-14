//! Core abstractions: Node, Flow, SharedStore, Batch, ParallelFlow.

pub mod batch;
pub mod error;
pub mod flow;
pub mod node;
pub mod parallel;
pub mod store;
pub mod typed_flow;
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
