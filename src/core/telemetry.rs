use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A context object that flows through nodes to accumulate telemetry metrics.
///
/// It tracks LLM token usage, total execution time, and individual node execution latencies.
#[derive(Debug, Clone)]
pub struct FlowContext {
    /// Total number of LLM tokens consumed during this flow.
    pub token_usage: usize,
    /// The time when the flow execution started.
    pub start_time: Instant,
    /// Execution duration broken down by node name.
    pub node_durations: HashMap<String, Duration>,
}

impl Default for FlowContext {
    fn default() -> Self {
        Self::new()
    }
}

impl FlowContext {
    /// Create a new, empty telemetry context and start the timer.
    pub fn new() -> Self {
        Self {
            token_usage: 0,
            start_time: Instant::now(),
            node_durations: HashMap::new(),
        }
    }

    /// Add to the total token count.
    pub fn add_tokens(&mut self, tokens: usize) {
        self.token_usage += tokens;
    }

    /// Record the duration for a specific node's execution.
    pub fn record_node_duration(&mut self, node_name: &str, duration: Duration) {
        let entry = self.node_durations.entry(node_name.to_string()).or_default();
        *entry += duration;
    }

    /// Get the total elapsed time since the context was created.
    pub fn total_elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}
