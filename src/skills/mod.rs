use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Represents a loaded Skill from a SKILL.md file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    #[serde(default)]
    pub instructions: String,
}

impl Skill {
    /// Parse a SKILL.md file containing YAML frontmatter and a Markdown body.
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        Self::from_str(&content)
    }

    /// Parse a SKILL.md content string.
    pub fn from_str(content: &str) -> Result<Self> {
        if !content.starts_with("---") {
            anyhow::bail!(
                "Invalid skill file format: Missing YAML frontmatter (must start with ---)"
            );
        }

        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() < 3 {
            anyhow::bail!("Invalid skill file format: Missing closing --- for YAML frontmatter");
        }

        let frontmatter = parts[1].trim();
        let body = parts[2].trim();

        let mut skill: Skill = serde_yaml::from_str(frontmatter)
            .context("Failed to parse YAML frontmatter in skill file")?;

        skill.instructions = body.to_string();

        Ok(skill)
    }
}
