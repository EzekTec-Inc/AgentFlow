use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentFlowError {
    NotFound(String),
    Timeout(String),
    NodeFailure(String),
    ExecutionLimitExceeded(String),
    TypeMismatch(String),
    Custom(String),
}

impl fmt::Display for AgentFlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentFlowError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AgentFlowError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            AgentFlowError::NodeFailure(msg) => write!(f, "Node failure: {}", msg),
            AgentFlowError::ExecutionLimitExceeded(msg) => write!(f, "Execution limit exceeded: {}", msg),
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
