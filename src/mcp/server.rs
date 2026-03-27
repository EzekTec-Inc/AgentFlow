use crate::core::error::AgentFlowError;
use crate::skills::Skill;
use rmcp::{
    handler::server::ServerHandler,
    model::{
        AnnotateAble, CallToolRequestParam, CallToolResult, Content, ListResourceTemplatesResult,
        ListResourcesResult, ListToolsResult, RawResource, ReadResourceRequestParam,
        ReadResourceResult, Resource, ResourceContents, ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
    transport::stdio,
    RoleServer, ServiceExt,
};
use serde_json::{json, Map, Value};
use std::{borrow::Cow, collections::BTreeSet, process::Stdio, sync::Arc, time::Duration};
use tokio::process::Child;
use tracing::{debug, warn};

/// Default timeout for MCP tool execution (30 seconds).
const MCP_TOOL_TIMEOUT: Duration = Duration::from_secs(30);
const RESOURCE_URI_PREFIX: &str = "agentflow://skill";
const BLOCKED_SHELL_COMMANDS: &[&str] = &[
    "sh",
    "bash",
    "cmd",
    "powershell",
    "pwsh",
    "zsh",
    "fish",
    "dash",
    "ash",
    "csh",
    "tcsh",
    "ksh",
];

async fn wait_with_timeout(
    child: Child,
    timeout: Duration,
) -> Result<std::process::Output, std::io::Error> {
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("process timed out after {}s", timeout.as_secs()),
        )),
    }
}

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
                resources: Some(Default::default()),
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

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
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

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        let resources = build_resources(&self.skills);
        debug!(count = resources.len(), "McpServer resources/list");
        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        let uri = request.uri.to_string();
        match render_resource_contents(&self.skills, &uri) {
            Some(text) => Ok(ReadResourceResult {
                contents: vec![ResourceContents::text(text, request.uri)],
            }),
            None => Err(rmcp::ErrorData::resource_not_found(
                "resource_not_found",
                Some(json!({ "uri": uri })),
            )),
        }
    }

    async fn list_resource_templates(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, rmcp::ErrorData> {
        Ok(ListResourceTemplatesResult {
            resource_templates: Vec::new(),
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let tool_name = request.name.as_ref();
        let arguments = request.arguments.unwrap_or_default();

        let matched_tool = self.skills.iter().find_map(|skill| {
            skill
                .tools
                .iter()
                .flatten()
                .find(|tool| tool.name == tool_name)
        });

        match matched_tool {
            Some(tool) => {
                debug!(tool = %tool.name, "McpServer executing tool");

                if let Err(message) = validate_tool_arguments(tool, &arguments) {
                    warn!(tool = %tool.name, error = %message, "McpServer invalid tool arguments");
                    return Ok(CallToolResult::error(vec![Content::text(message)]));
                }

                if is_blocked_shell_command(&tool.command) {
                    warn!(tool = %tool.name, command = %tool.command, "McpServer blocked shell-based tool execution");
                    return Ok(CallToolResult::error(vec![Content::text(format!(
                        "Tool '{}' uses blocked shell command '{}'",
                        tool.name, tool.command
                    ))]));
                }

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

                let result = match tokio::process::Command::new(&tool.command)
                    .args(&final_args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .kill_on_drop(true)
                    .spawn()
                {
                    Ok(child) => wait_with_timeout(child, MCP_TOOL_TIMEOUT).await,
                    Err(err) => Err(err),
                };

                match result {
                    Ok(output) => {
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
                    Err(err) if err.kind() != std::io::ErrorKind::TimedOut => {
                        Ok(CallToolResult::error(vec![Content::text(format!(
                            "Failed to execute tool '{}': {err}",
                            tool.name
                        ))]))
                    }
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
        ("additionalProperties".to_string(), Value::Bool(false)),
    ])
}

fn is_blocked_shell_command(command: &str) -> bool {
    let normalized = command.rsplit(['/', '\\']).next().unwrap_or(command);
    BLOCKED_SHELL_COMMANDS
        .iter()
        .any(|blocked| normalized.eq_ignore_ascii_case(blocked))
}

fn validate_tool_arguments(
    tool: &crate::skills::SkillTool,
    arguments: &Map<String, Value>,
) -> Result<(), String> {
    let placeholders = extract_placeholders(&tool.args);
    let missing = placeholders
        .iter()
        .filter(|name| !arguments.contains_key(name.as_str()))
        .cloned()
        .collect::<Vec<_>>();

    if !missing.is_empty() {
        return Err(format!(
            "Missing required input(s) for tool '{}': {}",
            tool.name,
            missing.join(", ")
        ));
    }

    let non_string = placeholders
        .iter()
        .filter_map(|name| match arguments.get(name) {
            Some(Value::String(_)) => None,
            Some(other) => Some(format!("{name} ({})", json_type_name(other))),
            None => None,
        })
        .collect::<Vec<_>>();

    if !non_string.is_empty() {
        return Err(format!(
            "Invalid input type(s) for tool '{}'; expected string for: {}",
            tool.name,
            non_string.join(", ")
        ));
    }

    Ok(())
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
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

fn build_resources(skills: &[Skill]) -> Vec<Resource> {
    let mut resources = Vec::new();

    for skill in skills {
        resources.push(make_resource(
            skill_resource_uri(skill),
            format!("{} skill", skill.name),
            Some(skill.description.clone()),
        ));

        for tool in skill.tools.iter().flatten() {
            resources.push(make_resource(
                tool_resource_uri(skill, &tool.name),
                format!("{} tool", tool.name),
                tool.description.clone(),
            ));
        }
    }

    resources
}

fn render_resource_contents(skills: &[Skill], uri: &str) -> Option<String> {
    let parsed = parse_resource_uri(uri)?;
    let skill = skills
        .iter()
        .find(|skill| slugify(&skill.name) == parsed.skill_slug)?;

    match parsed.kind {
        ResourceKind::Skill => Some(render_skill_resource(skill)),
        ResourceKind::Tool(tool_slug) => skill
            .tools
            .iter()
            .flatten()
            .find(|tool| slugify(&tool.name) == tool_slug)
            .map(|tool| render_tool_resource(skill, tool)),
    }
}

fn render_skill_resource(skill: &Skill) -> String {
    let tool_names = skill
        .tools
        .iter()
        .flatten()
        .map(|tool| format!("- {}", tool.name))
        .collect::<Vec<_>>();

    format!(
        "Skill: {}\nVersion: {}\nDescription: {}\nTools:\n{}",
        skill.name,
        skill.version.as_deref().unwrap_or("unknown"),
        skill.description,
        if tool_names.is_empty() {
            "- none".to_string()
        } else {
            tool_names.join("\n")
        }
    )
}

fn render_tool_resource(skill: &Skill, tool: &crate::skills::SkillTool) -> String {
    let placeholders = extract_placeholders(&tool.args);
    format!(
        "Skill: {}\nTool: {}\nDescription: {}\nCommand: {}\nArgs: {}\nPlaceholders: {}",
        skill.name,
        tool.name,
        tool.description.as_deref().unwrap_or(""),
        tool.command,
        serde_json::to_string(&tool.args).unwrap_or_else(|_| "[]".to_string()),
        if placeholders.is_empty() {
            "none".to_string()
        } else {
            placeholders.join(", ")
        }
    )
}

fn make_resource(uri: String, name: String, description: Option<String>) -> Resource {
    let mut resource = RawResource::new(uri, name);
    resource.description = description;
    resource.no_annotation()
}

fn skill_resource_uri(skill: &Skill) -> String {
    format!("{}/{}/overview", RESOURCE_URI_PREFIX, slugify(&skill.name))
}

fn tool_resource_uri(skill: &Skill, tool_name: &str) -> String {
    format!(
        "{}/{}/tool/{}",
        RESOURCE_URI_PREFIX,
        slugify(&skill.name),
        slugify(tool_name)
    )
}

fn slugify(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

struct ParsedResourceUri {
    skill_slug: String,
    kind: ResourceKind,
}

enum ResourceKind {
    Skill,
    Tool(String),
}

fn parse_resource_uri(uri: &str) -> Option<ParsedResourceUri> {
    let remainder = uri.strip_prefix(&format!("{}/", RESOURCE_URI_PREFIX))?;
    let parts = remainder.split('/').collect::<Vec<_>>();
    match parts.as_slice() {
        [skill_slug, "overview"] => Some(ParsedResourceUri {
            skill_slug: (*skill_slug).to_string(),
            kind: ResourceKind::Skill,
        }),
        [skill_slug, "tool", tool_slug] => Some(ParsedResourceUri {
            skill_slug: (*skill_slug).to_string(),
            kind: ResourceKind::Tool((*tool_slug).to_string()),
        }),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{
        build_resources, extract_placeholders, render_resource_contents, skill_resource_uri,
        tool_input_schema, tool_resource_uri, validate_tool_arguments,
    };
    use crate::skills::{Skill, SkillTool};
    use serde_json::{json, Map, Value};

    fn sample_skill() -> Skill {
        Skill {
            name: "GoA Research Tools".to_string(),
            description: "Research workflow utilities".to_string(),
            version: Some("1.0.0".to_string()),
            instructions: "Use the tools carefully".to_string(),
            tools: Some(vec![sample_tool()]),
        }
    }

    fn sample_tool() -> SkillTool {
        SkillTool {
            name: "fetch_url".to_string(),
            description: Some("Fetch a URL".to_string()),
            command: "curl".to_string(),
            args: vec![
                "-sL".to_string(),
                "{{url}}".to_string(),
                "{{output_path}}".to_string(),
            ],
        }
    }

    #[test]
    fn extracts_unique_placeholders_in_sorted_order() {
        let placeholders = extract_placeholders(&[
            "{{output_path}}".to_string(),
            "{{url}}".to_string(),
            "prefix-{{url}}-suffix".to_string(),
        ]);

        assert_eq!(
            placeholders,
            vec!["output_path".to_string(), "url".to_string()]
        );
    }

    #[test]
    fn builds_schema_with_required_string_properties() {
        let schema = tool_input_schema(&sample_tool());

        assert_eq!(
            schema.get("type"),
            Some(&Value::String("object".to_string()))
        );
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&Value::Bool(false))
        );

        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties object");
        assert_eq!(properties.len(), 2);
        assert_eq!(
            properties["url"]["type"],
            Value::String("string".to_string())
        );
        assert_eq!(
            properties["output_path"]["type"],
            Value::String("string".to_string())
        );

        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .expect("required array");
        assert_eq!(required, &vec![json!("output_path"), json!("url")]);
    }

    #[test]
    fn rejects_missing_required_placeholder_arguments() {
        let mut arguments = Map::new();
        arguments.insert(
            "url".to_string(),
            Value::String("https://example.com".to_string()),
        );

        let error = validate_tool_arguments(&sample_tool(), &arguments).unwrap_err();
        assert!(error.contains("Missing required input(s)"));
        assert!(error.contains("output_path"));
    }

    #[test]
    fn rejects_non_string_placeholder_arguments() {
        let mut arguments = Map::new();
        arguments.insert(
            "url".to_string(),
            Value::String("https://example.com".to_string()),
        );
        arguments.insert("output_path".to_string(), json!(42));

        let error = validate_tool_arguments(&sample_tool(), &arguments).unwrap_err();
        assert!(error.contains("Invalid input type(s)"));
        assert!(error.contains("output_path (number)"));
    }

    #[test]
    fn accepts_valid_string_arguments_and_extra_fields() {
        let mut arguments = Map::new();
        arguments.insert(
            "url".to_string(),
            Value::String("https://example.com".to_string()),
        );
        arguments.insert(
            "output_path".to_string(),
            Value::String("/tmp/report.txt".to_string()),
        );
        arguments.insert("extra".to_string(), Value::Bool(true));

        assert_eq!(validate_tool_arguments(&sample_tool(), &arguments), Ok(()));
    }

    #[test]
    fn builds_skill_and_tool_resources() {
        let skill = sample_skill();
        let resources = build_resources(&[skill]);

        assert_eq!(resources.len(), 2);
        assert_eq!(
            resources[0].uri.as_str(),
            "agentflow://skill/goa-research-tools/overview"
        );
        assert_eq!(
            resources[1].uri.as_str(),
            "agentflow://skill/goa-research-tools/tool/fetch-url"
        );
    }

    #[test]
    fn renders_skill_resource_contents() {
        let skill = sample_skill();
        let uri = skill_resource_uri(&skill);
        let content = render_resource_contents(&[skill], &uri).expect("skill resource content");

        assert!(content.contains("Skill: GoA Research Tools"));
        assert!(content.contains("Tools:"));
        assert!(content.contains("- fetch_url"));
    }

    #[test]
    fn renders_tool_resource_contents() {
        let skill = sample_skill();
        let uri = tool_resource_uri(&skill, "fetch_url");
        let content = render_resource_contents(&[skill], &uri).expect("tool resource content");

        assert!(content.contains("Tool: fetch_url"));
        assert!(content.contains("Command: curl"));
        assert!(content.contains("Placeholders: output_path, url"));
    }
}
