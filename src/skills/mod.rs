use crate::core::error::AgentFlowError;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Represents an executable tool defined in a skill.
///
/// Security: skill tools are trusted executable configuration. Do not load
/// untrusted skill files, and do not define shell-interpreter commands such as
/// `sh`, `bash`, `cmd`, `powershell`, or `pwsh` with placeholder-based input.
pub struct SkillTool {
    /// Tool name.
    pub name: String,
    /// Optional tool description.
    pub description: Option<String>,
    /// System command to execute.
    pub command: String,
    #[serde(default)]
    /// Arguments to pass to the command.
    pub args: Vec<String>,
}

/// Represents a loaded Skill from a SKILL.md file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Skill name.
    pub name: String,
    /// Skill description.
    pub description: String,
    /// Optional skill version.
    pub version: Option<String>,
    /// Optional list of tools provided by this skill.
    pub tools: Option<Vec<SkillTool>>,
    #[serde(default)]
    /// Base instructions or prompt for the skill.
    pub instructions: String,
}

impl Skill {
    /// Parse a SKILL.md file containing YAML frontmatter and a Markdown body.
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, AgentFlowError> {
        let content = tokio::fs::read_to_string(path).await?;
        Self::parse(&content)
    }

    /// Parse a SKILL.md content string.
    pub fn parse(content: &str) -> Result<Self, AgentFlowError> {
        if !content.starts_with("---") {
            return Err(AgentFlowError::Custom(
                "Invalid skill file format: Missing YAML frontmatter (must start with ---)"
                    .to_string(),
            ));
        }

        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() < 3 {
            return Err(AgentFlowError::Custom(
                "Invalid skill file format: Missing closing --- for YAML frontmatter".to_string(),
            ));
        }

        let frontmatter = parts[1].trim();
        let body = parts[2].trim();

        let mut skill: Skill = serde_yaml::from_str(frontmatter).map_err(|e| {
            AgentFlowError::Custom(format!(
                "Failed to parse YAML frontmatter in skill file: {}",
                e
            ))
        })?;

        skill.instructions = body.to_string();

        Ok(skill)
    }
}
