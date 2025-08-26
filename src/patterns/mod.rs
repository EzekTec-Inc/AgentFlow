//! High-level design patterns built on the core abstractions.

pub mod agent;
pub mod workflow;
pub mod rag;
pub mod multi_agent;
pub mod mapreduce;
pub mod structured_output;
pub mod batchflow;

// Re-export all patterns for convenience
pub use agent::Agent;
pub use workflow::Workflow;
pub use rag::Rag;
pub use multi_agent::MultiAgent;
pub use mapreduce::MapReduce;
pub use structured_output::StructuredOutput;
pub use batchflow::BatchFlow;
