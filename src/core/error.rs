use std::fmt;

/// Unified error type for all AgentFlow operations.
///
/// Variants are designed to be actionable — callers can match on the variant
/// to decide whether to retry, abort, or surface to the user.
///
/// # Examples
///
/// ```rust
/// use agentflow::core::error::AgentFlowError;
///
/// let err = AgentFlowError::NotFound("prompt key missing".into());
/// assert_eq!(err.to_string(), "Not found: prompt key missing");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentFlowError {
    /// A required key or resource was not found in the store or registry.
    NotFound(String),
    /// An operation timed out. Treated as **transient** by [`Agent::decide_result`]
    /// — it will be retried up to `max_retries` times.
    ///
    /// [`Agent::decide_result`]: crate::patterns::agent::Agent::decide_result
    Timeout(String),
    /// A node produced an unrecoverable failure. Treated as **fatal** by
    /// [`Agent::decide_result`] — retries are skipped and the error is returned
    /// immediately.
    ///
    /// [`Agent::decide_result`]: crate::patterns::agent::Agent::decide_result
    NodeFailure(String),
    /// [`Flow::run_safe`] or [`TypedFlow::run_safe`] reached the `max_steps` limit.
    ///
    /// [`Flow::run_safe`]: crate::core::flow::Flow::run_safe
    /// [`TypedFlow::run_safe`]: crate::core::typed_flow::TypedFlow::run_safe
    ExecutionLimitExceeded(String),
    /// A value in the store had an unexpected type (e.g. expected `i64`, found `String`).
    TypeMismatch(String),
    /// Any other error that doesn't fit a specific variant above.
    Custom(String),
}

impl fmt::Display for AgentFlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentFlowError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AgentFlowError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            AgentFlowError::NodeFailure(msg) => write!(f, "Node failure: {}", msg),
            AgentFlowError::ExecutionLimitExceeded(msg) => {
                write!(f, "Execution limit exceeded: {}", msg)
            }
            AgentFlowError::TypeMismatch(msg) => write!(f, "Type mismatch: {}", msg),
            AgentFlowError::Custom(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for AgentFlowError {}

impl From<std::io::Error> for AgentFlowError {
    fn from(error: std::io::Error) -> Self {
        AgentFlowError::Custom(format!("IO Error: {}", error))
    }
}

impl From<serde_json::Error> for AgentFlowError {
    fn from(error: serde_json::Error) -> Self {
        AgentFlowError::Custom(format!("JSON Error: {}", error))
    }
}

impl From<anyhow::Error> for AgentFlowError {
    /// Convert an [`anyhow::Error`] into [`AgentFlowError::Custom`].
    ///
    /// This preserves the full error chain via [`anyhow::Error`]'s `Display`
    /// impl (which prints the chain as `"outer: inner: cause"`), so no
    /// diagnostic information is lost.
    ///
    /// # Example
    ///
    /// ```rust
    /// use agentflow::core::error::AgentFlowError;
    /// use anyhow::anyhow;
    ///
    /// let err: AgentFlowError = anyhow!("something went wrong").into();
    /// assert!(err.to_string().contains("something went wrong"));
    /// ```
    fn from(error: anyhow::Error) -> Self {
        AgentFlowError::Custom(format!("Error: {}", error))
    }
}
