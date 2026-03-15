#[cfg(feature = "skills")]
use crate::core::error::AgentFlowError;
#[cfg(feature = "skills")]
use crate::core::node::{NodeResult, SharedStore};
#[cfg(feature = "skills")]
use crate::skills::{Skill, SkillTool};
#[cfg(feature = "skills")]
use std::future::Future;
#[cfg(feature = "skills")]
use std::pin::Pin;
#[cfg(feature = "skills")]
use tokio::process::Command;
#[cfg(feature = "skills")]
use tracing::{info, instrument, warn};

#[cfg(feature = "skills")]
/// A pattern that wraps a `SkillTool` into a `NodeResult` executor.
///
/// `SkillToolNode` takes a defined tool from a parsed SKILL.md file and
/// executes it locally as a standard node in the orchestrator flow. 
///
/// It writes the output to `store["tool_stdout"]`, `store["tool_stderr"]`, 
/// and `store["tool_exit_code"]`.
#[derive(Clone)]
pub struct SkillToolNode {
    /// The skill tool to execute.
    pub tool: SkillTool,
    /// Timeout in seconds (default: 60)
    pub timeout_secs: u64,
}

#[cfg(feature = "skills")]
impl SkillToolNode {
    /// Create a new `SkillToolNode` from a `SkillTool`.
    pub fn new(tool: SkillTool) -> Self {
        Self { tool, timeout_secs: 60 }
    }

    /// Set a custom execution timeout in seconds.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

#[cfg(feature = "skills")]
impl NodeResult<SharedStore, SharedStore> for SkillToolNode {
    #[instrument(name = "skill_tool.call", skip(self, input), fields(tool_name = %self.tool.name))]
    fn call(
        &self,
        input: SharedStore,
    ) -> Pin<Box<dyn Future<Output = Result<SharedStore, AgentFlowError>> + Send + '_>> {
        let tool = self.tool.clone();
        let timeout_secs = self.timeout_secs;

        Box::pin(async move {
            info!("Executing skill tool: {}", tool.name);
            let mut cmd = Command::new(&tool.command);
            cmd.args(&tool.args);

            match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), cmd.output()).await {
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let status = output.status.code().unwrap_or(-1);

                    let mut store = input.write().await;
                    store.insert("tool_stdout".into(), serde_json::json!(stdout));
                    store.insert("tool_stderr".into(), serde_json::json!(stderr));
                    store.insert("tool_exit_code".into(), serde_json::json!(status));

                    Ok(input.clone())
                }
                Ok(Err(e)) => {
                    warn!("Skill tool execution failed: {}", e);
                    Err(AgentFlowError::Custom(format!(
                        "Failed to run skill tool {}: {}",
                        tool.name, e
                    )))
                }
                Err(_) => {
                    warn!("Skill tool execution timed out");
                    Err(AgentFlowError::Timeout(format!(
                        "Tool {} timed out after {}s",
                        tool.name, timeout_secs
                    )))
                }
            }
        })
    }
}

#[cfg(feature = "skills")]
/// A pattern that injects a `Skill`'s instructions into the state context.
///
/// `SkillInjector` takes a `Skill` and simply merges its base instructions 
/// into the store so that subsequent LLM nodes can read it and use it as 
/// a system preamble or context guide.
#[derive(Clone)]
pub struct SkillInjector {
    /// The skill to inject.
    pub skill: Skill,
    /// The state key where instructions will be stored (default: `"skill_instructions"`).
    pub key: String,
}

#[cfg(feature = "skills")]
impl SkillInjector {
    /// Create a new `SkillInjector` for the given skill.
    pub fn new(skill: Skill) -> Self {
        Self { 
            skill, 
            key: "skill_instructions".to_string(),
        }
    }

    /// Set the destination key in the store.
    pub fn with_key(mut self, key: &str) -> Self {
        self.key = key.to_string();
        self
    }
}

#[cfg(feature = "skills")]
impl crate::core::node::Node<SharedStore, SharedStore> for SkillInjector {
    #[instrument(name = "skill_injector.call", skip(self, input), fields(skill_name = %self.skill.name))]
    fn call(
        &self,
        input: SharedStore,
    ) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
        let instructions = self.skill.instructions.clone();
        let key = self.key.clone();
        
        Box::pin(async move {
            info!("Injecting skill instructions for: {}", self.skill.name);
            let mut store = input.write().await;
            store.insert(key, serde_json::json!(instructions));
            input.clone()
        })
    }
}
