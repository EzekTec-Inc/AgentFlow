//! High-level design patterns built on the core abstractions.

pub mod agent;
pub mod batchflow;
pub mod mapreduce;
pub mod multi_agent;
pub mod rag;
pub mod rpi;
pub mod structured_output;
pub mod workflow;

// Re-export all patterns for convenience
pub use agent::Agent;
pub use batchflow::BatchFlow;
pub use mapreduce::MapReduce;
pub use multi_agent::MultiAgent;
pub use rag::Rag;
pub use rpi::RpiWorkflow;
pub use structured_output::StructuredOutput;
pub use workflow::Workflow;
