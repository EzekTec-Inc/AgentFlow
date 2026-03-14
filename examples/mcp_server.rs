use agentflow::core::error::AgentFlowError;
use agentflow::mcp::server::McpServer;
use agentflow::skills::Skill;

#[tokio::main]
async fn main() -> Result<(), AgentFlowError> {
    let skill_content = r#"---
name: GoAResearchTools
description: Tools for crawling the Government of Alberta design system and generating reports.
version: 1.0.0
tools:
  - name: crawl_goa_url
    description: Fetches content from a Government of Alberta Design System URL (e.g., https://design.alberta.ca)
    command: curl
    args: ["-sL"]
  - name: generate_pdf
    description: Generates a PDF report from markdown content. Pass the content as the first argument.
    command: bash
    args: ["-c", "cat << 'EOF' > report.md\n$1\nEOF\necho '% GoA Design System Report' > report.pdf && echo 'PDF generation mocked successfully.'"]
---
You are a tool server providing capabilities for the GoA research pipeline.
"#;
    let skill = Skill::parse(skill_content)?;

    let server = McpServer::new("GoA_Research_Server", "0.2.0")
        .register_skill(skill);

    server.run().await?;
    
    Ok(())
}
