use crate::core::error::AgentFlowError;
use crate::skills::Skill;
use rmcp::{
    handler::server::ServerHandler,
    model::{
        CallToolRequestParam, CallToolResult, Content, ListToolsResult, ServerCapabilities,
        ServerInfo, Tool,
    },
    service::RequestContext,
    transport::stdio,
    RoleServer, ServiceExt,
};
use serde_json::{Map, Value};
use std::{
    borrow::Cow,
    collections::BTreeSet,
    sync::Arc,
    time::Duration,
};
use tracing::{debug, warn};

/// Default timeout for MCP tool execution (30 seconds).
const MCP_TOOL_TIMEOUT: Duration = Duration::from_secs(30);

/// rmcp-backed MCP server for AgentFlow skills.
pub struct McpServer {
    name: String,
    version: String,
    skills: Vec<Skill>,
}

impl McpServer {
    /// Create a new `McpServer` instance.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            skills: Vec::new(),
        }
    }

    /// Register a loaded `Skill` with this server so it appears in `tools/list`
    /// and can be dispatched via `tools/call`.
    pub fn register_skill(mut self, skill: Skill) -> Self {
        self.skills.push(skill);
        self
    }

    /// Runs the server over stdio using rmcp's protocol implementation.
    pub async fn run(self) -> Result<(), AgentFlowError> {
        let server = self;
        let service = server
            .serve(stdio())
            .await
            .map_err(|e| AgentFlowError::Custom(format!("Failed to start MCP server: {e}")))?;

        service
            .waiting()
            .await
            .map_err(|e| AgentFlowError::Custom(format!("MCP server exited with error: {e}")))?;

        Ok(())
    }
}

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities {
                tools: Some(Default::default()),
                ..ServerCapabilities::default()
            },
            server_info: rmcp::model::Implementation {
                name: self.name.clone(),
                version: self.version.clone(),
            },
            ..ServerInfo::default()
        }
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_ {
        async move {
            let tools: Vec<Tool> = self
                .skills
                .iter()
                .flat_map(|skill| skill.tools.iter().flatten())
                .map(|tool| {
                    let description = tool.description.clone().map(Cow::Owned);
                    Tool {
                        name: Cow::Owned(tool.name.clone()),
                        description,
                        input_schema: Arc::new(tool_input_schema(tool)),
                        annotations: None,
                    }
                })
                .collect();

            debug!(count = tools.len(), "McpServer tools/list");
            Ok(ListToolsResult::with_all_items(tools))
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::ErrorData>> + Send + '_ {
        async move {
            let tool_name = request.name.as_ref();
            let arguments = request.arguments.unwrap_or_default();

            let matched_tool = self.skills.iter().find_map(|skill| {
                skill.tools
                    .iter()
                    .flatten()
                    .find(|tool| tool.name == tool_name)
            });

            match matched_tool {
                Some(tool) => {
                    debug!(tool = %tool.name, "McpServer executing tool");

                    let mut final_args = Vec::new();
                    for arg in &tool.args {
                        let mut modified_arg = arg.clone();
                        for (k, v) in &arguments {
                            let val_str = match v {
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            modified_arg = modified_arg.replace(&format!("{{{{{}}}}}", k), &val_str);
                        }
                        final_args.push(modified_arg);
                    }

                    let exec = tokio::process::Command::new(&tool.command)
                        .args(&final_args)
                        .output();

                    match tokio::time::timeout(MCP_TOOL_TIMEOUT, exec).await {
                        Ok(Ok(output)) => {
                            let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
                            let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
                            let status = output.status.code().unwrap_or(-1);

                            let payload = serde_json::json!({
                                "stdout": stdout_str,
                                "stderr": stderr_str,
                                "exitCode": status,
                            });

                            let content = Content::json(payload).unwrap_or_else(|_| {
                                Content::text(format!(
                                    "tool={} exitCode={} stderr={} stdout={}",
                                    tool.name, status, stderr_str, stdout_str
                                ))
                            });

                            Ok(if output.status.success() {
                                CallToolResult::success(vec![content])
                            } else {
                                CallToolResult::error(vec![content])
                            })
                        }
                        Ok(Err(err)) => Ok(CallToolResult::error(vec![Content::text(format!(
                            "Failed to execute tool '{}': {err}",
                            tool.name
                        ))])),
                        Err(_) => Ok(CallToolResult::error(vec![Content::text(format!(
                            "Tool '{}' timed out after {} seconds",
                            tool.name,
                            MCP_TOOL_TIMEOUT.as_secs()
                        ))])),
                    }
                }
                None => {
                    warn!(tool = tool_name, "McpServer tool not found");
                    Ok(CallToolResult::error(vec![Content::text(format!(
                        "Tool not found: {tool_name}"
                    ))]))
                }
            }
        }
    }
}

fn tool_input_schema(tool: &crate::skills::SkillTool) -> Map<String, Value> {
    let placeholders = extract_placeholders(&tool.args);
    let properties = placeholders
        .iter()
        .map(|name| {
            (
                name.clone(),
                Value::Object(Map::from_iter([(
                    "type".to_string(),
                    Value::String("string".to_string()),
                )])),
            )
        })
        .collect::<Map<String, Value>>();

    Map::from_iter([
        ("type".to_string(), Value::String("object".to_string())),
        ("properties".to_string(), Value::Object(properties)),
        (
            "required".to_string(),
            Value::Array(placeholders.into_iter().map(Value::String).collect()),
        ),
        (
            "additionalProperties".to_string(),
            Value::Bool(true),
        ),
    ])
}

fn extract_placeholders(args: &[String]) -> Vec<String> {
    let mut placeholders = BTreeSet::new();

    for arg in args {
        let mut rest = arg.as_str();
        while let Some(start) = rest.find("{{") {
            rest = &rest[start + 2..];
            let Some(end) = rest.find("}}") else {
                break;
            };

            let key = rest[..end].trim();
            if !key.is_empty() {
                placeholders.insert(key.to_string());
            }
            rest = &rest[end + 2..];
        }
    }

    placeholders.into_iter().collect()
}
