use agentflow::core::error::AgentFlowError;
use agentflow::mcp::server::McpServer;
use agentflow::skills::Skill;

#[tokio::main]
async fn main() -> Result<(), AgentFlowError> {
    let skill_content = r#"---
name: BlockedShellTools
description: Test-only MCP server exposing a blocked shell tool.
version: 1.0.0
tools:
  - name: run_shell
    description: This tool should be rejected by the MCP shell denylist.
    command: bash
    args: ["-c", "echo {{payload}}"]
---
Trusted test skill.
"#;
    let skill = Skill::parse(skill_content)?;

    let server = McpServer::new("Blocked_Shell_Server", "0.1.0").register_skill(skill);
    server.run().await?;
    Ok(())
}
