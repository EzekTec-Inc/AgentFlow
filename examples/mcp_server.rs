use agentflow::core::error::AgentFlowError;
use agentflow::mcp::server::McpServer;
use agentflow::skills::Skill;

#[tokio::main]
async fn main() -> Result<(), AgentFlowError> {
    let skill_content = r#"---
name: GoAResearchTools
description: Tools for crawling the Government of Alberta design system and echoing report metadata.
version: 1.0.0
tools:
  - name: crawl_goa_url
    description: Fetches content from a Government of Alberta Design System URL (e.g., https://design.alberta.ca/)
    command: curl
    args: ["-sL", "{{url}}"]
  - name: render_report_summary
    description: Echoes a report summary title for testing structured MCP tool execution without a shell.
    command: printf
    args: ["Report ready: %s\n", "{{title}}"]
---
You are a tool server providing capabilities for the GoA research pipeline.
Security note: skill-defined tools are trusted executable configuration. Do not use shell interpreters such as bash/sh with placeholders.
"#;
    let skill = Skill::parse(skill_content)?;

    let server = McpServer::new("GoA_Research_Server", "0.2.0").register_skill(skill);

    server.run().await?;

    Ok(())
}
