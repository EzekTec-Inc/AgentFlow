use agentflow::core::error::AgentFlowError;
use agentflow::mcp::McpServer;
use agentflow::skills::{Skill, SkillTool};
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<(), AgentFlowError> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(Level::DEBUG)
        .init();
        
    info!("Starting AgentFlow MCP Server...");

    let example_skill = Skill {
        name: "system_info".to_string(),
        description: "A set of tools for retrieving basic system information".to_string(),
        version: Some("1.0.0".to_string()),
        instructions: "Use these tools to gather system info.".to_string(),
        tools: Some(vec![
            SkillTool {
                name: "echo_tool".to_string(),
                description: Some("Echoes back the provided arguments".to_string()),
                command: "echo".to_string(),
                args: vec![],
            },
            SkillTool {
                name: "date_tool".to_string(),
                description: Some("Returns the current date and time".to_string()),
                command: "date".to_string(),
                args: vec![],
            },
        ]),
    };

    let server = McpServer::new("AgentFlow_System_MCP", "0.1.0")
        .register_skill(example_skill);

    server.run().await?;
    Ok(())
}
