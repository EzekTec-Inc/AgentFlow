use crate::core::error::AgentFlowError;
use crate::skills::Skill;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, warn};

/// Default timeout for MCP tool execution (30 seconds).
const MCP_TOOL_TIMEOUT: Duration = Duration::from_secs(30);

/// Minimalist MCP (Model Context Protocol) Server for AgentFlow.
/// This reads JSON-RPC requests from stdin, processes them, and writes
/// JSON-RPC responses to stdout.
pub struct McpServer {
    name: String,
    version: String,
    skills: Vec<Skill>,
}

impl McpServer {
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

    /// Runs the server loop, reading from stdin and writing to stdout.
    /// Uses async I/O throughout — no blocking calls on Tokio worker threads.
    pub async fn run(&self) -> Result<(), AgentFlowError> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut buffer = String::new();

        loop {
            buffer.clear();
            let bytes_read = reader.read_line(&mut buffer).await?;
            if bytes_read == 0 {
                break; // EOF
            }

            let line = buffer.trim();
            if line.is_empty() {
                continue;
            }

            if let Ok(request) = serde_json::from_str::<Value>(line) {
                if let Some(id) = request.get("id") {
                    let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");
                    debug!(method, "McpServer received request");

                    let response = match method {
                        "initialize" => {
                            json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "protocolVersion": "2024-11-05",
                                    "serverInfo": {
                                        "name": self.name,
                                        "version": self.version
                                    },
                                    "capabilities": {
                                        "tools": {}
                                    }
                                }
                            })
                        }
                        "tools/list" => {
                            let tools: Vec<Value> = self
                                .skills
                                .iter()
                                .flat_map(|skill| {
                                    skill.tools.iter().flatten().map(move |tool| {
                                        json!({
                                            "name": tool.name,
                                            "description": tool.description,
                                            "inputSchema": {
                                                "type": "object",
                                                "properties": {},
                                                "required": []
                                            }
                                        })
                                    })
                                })
                                .collect();
                            debug!(count = tools.len(), "McpServer tools/list");
                            json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "tools": tools
                                }
                            })
                        }
                        "tools/call" => {
                            let params = request.get("params");
                            let tool_name = params
                                .and_then(|p| p.get("name"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            // Find the matching SkillTool across all registered skills
                            let matched_tool = self.skills.iter().find_map(|skill| {
                                skill.tools.iter().flatten().find(|t| t.name == tool_name)
                            });

                            match matched_tool {
                                Some(tool) => {
                                    debug!(tool = %tool.name, "McpServer executing tool");

                                    let exec = tokio::process::Command::new(&tool.command)
                                        .args(&tool.args)
                                        .output();

                                    match tokio::time::timeout(MCP_TOOL_TIMEOUT, exec).await {
                                        Ok(Ok(output)) => {
                                            let stdout_str =
                                                String::from_utf8_lossy(&output.stdout).to_string();
                                            let stderr_str =
                                                String::from_utf8_lossy(&output.stderr).to_string();
                                            let status = output.status.code().unwrap_or(-1);
                                            json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "result": {
                                                    "content": [
                                                        {
                                                            "type": "text",
                                                            "text": stdout_str
                                                        }
                                                    ],
                                                    "stderr": stderr_str,
                                                    "exitCode": status
                                                }
                                            })
                                        }
                                        Ok(Err(e)) => {
                                            warn!(tool = %tool.name, error = %e, "McpServer tool execution failed");
                                            json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "error": {
                                                    "code": -32000,
                                                    "message": format!("Tool execution failed: {}", e)
                                                }
                                            })
                                        }
                                        Err(_elapsed) => {
                                            warn!(tool = %tool.name, "McpServer tool timed out");
                                            json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "error": {
                                                    "code": -32000,
                                                    "message": format!(
                                                        "Tool '{}' timed out after {}s",
                                                        tool.name,
                                                        MCP_TOOL_TIMEOUT.as_secs()
                                                    )
                                                }
                                            })
                                        }
                                    }
                                }
                                None => {
                                    warn!(tool = %tool_name, "McpServer unknown tool requested");
                                    json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "error": {
                                            "code": -32601,
                                            "message": format!("Unknown tool: {}", tool_name)
                                        }
                                    })
                                }
                            }
                        }
                        _ => {
                            json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": {
                                    "code": -32601,
                                    "message": "Method not found"
                                }
                            })
                        }
                    };

                    let mut response_str = serde_json::to_string(&response)?;
                    response_str.push('\n');
                    stdout.write_all(response_str.as_bytes()).await?;
                    stdout.flush().await?;
                }
            } else {
                // Invalid JSON
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": {
                        "code": -32700,
                        "message": "Parse error"
                    }
                });
                let mut response_str = serde_json::to_string(&response)?;
                response_str.push('\n');
                stdout.write_all(response_str.as_bytes()).await?;
                stdout.flush().await?;
            }
        }

        Ok(())
    }
}
