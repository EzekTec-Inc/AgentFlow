//! High-level design patterns built on the core abstractions.

/// Single-function agent pattern.
pub mod agent;
/// Batch execution pattern.
pub mod batchflow;
/// Map-reduce pattern.
pub mod mapreduce;
/// Multi-agent concurrent pattern.
pub mod multi_agent;
/// Retrieval-augmented generation pattern.
pub mod rag;
/// Retry-with-prompt-injection pattern.
pub mod rpi;
/// Structured output enforcement pattern.
pub mod structured_output;
/// Linear workflow pattern.
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
