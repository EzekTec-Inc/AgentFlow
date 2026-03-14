use agentflow::core::error::AgentFlowError;
use agentflow::mcp::server::McpServer;
use agentflow::skills::Skill;

#[tokio::main]
async fn main() -> Result<(), AgentFlowError> {
    // 1. Showcase AgentFlow's native ability to parse Markdown/YAML skills
    let sys_skill_content = r#"---
name: SystemOps
description: Core system operations
version: 1.0.0
tools:
  - name: execute_shell
    description: Run a bash command safely
    command: bash
    args: ["-c"]
---
You are a system operations tool.
"#;
    let sys_skill = Skill::parse(sys_skill_content)?;

    // 2. Initialize AgentFlow's built-in native MCP Server
    let server = McpServer::new("AgentFlow_Demo_Server", "0.2.0")
        // 3. Register the parsed skill directly
        .register_skill(sys_skill);

    // 4. Run the native asynchronous stdin/stdout loop
    // This automatically handles initialization, tools/list, and tools/call
    server.run().await?;
    
    Ok(())
}
