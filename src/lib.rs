#![doc = r#"
AgentFlow - A 100-line minimalist LLM framework

Core philosophy: Graph + Shared Store
- Node handles simple (LLM) tasks
- Flow connects nodes through Actions (labeled edges)
- Shared Store enables communication between nodes
- Batch nodes/flows allow for data-intensive tasks
- Async nodes/flows allow waiting for asynchronous tasks
"#]

pub mod core;
pub mod patterns;
pub mod utils;

/// Re-export the public API to match Python AgentFlow structure
pub mod prelude {
    pub use crate::core::node::{Node, SharedStore, SimpleNode, create_node, create_batch_node};
    pub use crate::core::flow::Flow;
    pub use crate::core::batch::{Batch, ParallelBatch};
    pub use crate::patterns::agent::Agent;
    pub use crate::patterns::workflow::Workflow;
    pub use crate::patterns::rag::Rag;
    pub use crate::patterns::mapreduce::MapReduce;
    pub use crate::patterns::multi_agent::MultiAgent;
    pub use crate::patterns::structured_output::StructuredOutput;
}

// Direct exports to match Python's flat namespace
pub use crate::core::node::{Node, SharedStore, SimpleNode, create_node, create_batch_node};
pub use crate::core::flow::Flow;
pub use crate::core::batch::{Batch, ParallelBatch};
pub use crate::patterns::agent::Agent;
pub use crate::patterns::workflow::Workflow;
pub use crate::patterns::rag::Rag;
pub use crate::patterns::mapreduce::MapReduce;
pub use crate::patterns::multi_agent::MultiAgent;
pub use crate::patterns::structured_output::StructuredOutput;
