use crate::core::error::AgentFlowError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// Options used to initialize an [`McpClient`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpClientOptions {
    /// Client display name sent during MCP `initialize`.
    pub client_name: String,
    /// Client version sent during MCP `initialize`.
    pub client_version: String,
}

impl Default for McpClientOptions {
    fn default() -> Self {
        Self {
            client_name: "agentflow".to_string(),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Minimal MCP tool descriptor returned by `tools/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// Tool name.
    pub name: String,
    /// Human-readable tool description.
    pub description: String,
    /// JSON Schema for tool input.
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Minimal MCP `tools/call` result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCallResult {
    /// Structured MCP content entries.
    pub content: Vec<Value>,
    /// Captured stderr, if any.
    #[serde(default)]
    pub stderr: String,
    /// Process exit code, if the server supplied one.
    #[serde(rename = "exitCode")]
    pub exit_code: Option<i64>,
}

/// Sequential stdio MCP client for the current AgentFlow newline-delimited server.
pub struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout_lines: Lines<BufReader<ChildStdout>>,
    next_id: u64,
    initialized: bool,
    server_name: Option<String>,
    server_version: Option<String>,
}

impl McpClient {
    /// Spawn a stdio MCP server process with piped stdin/stdout.
    pub fn spawn_stdio(mut command: Command) -> Result<Self, AgentFlowError> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = command.spawn().map_err(|e| {
            AgentFlowError::Custom(format!("Failed to spawn MCP server process: {e}"))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            AgentFlowError::Custom("Spawned MCP process did not expose stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AgentFlowError::Custom("Spawned MCP process did not expose stdout".to_string())
        })?;

        Ok(Self {
            child,
            stdin,
            stdout_lines: BufReader::new(stdout).lines(),
            next_id: 1,
            initialized: false,
            server_name: None,
            server_version: None,
        })
    }

    /// Send the MCP `initialize` request and record returned server metadata.
    pub async fn initialize(
        &mut self,
        options: McpClientOptions,
    ) -> Result<(), AgentFlowError> {
        let result = self
            .request(
                "initialize",
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {
                        "name": options.client_name,
                        "version": options.client_version,
                    }
                }),
            )
            .await?;

        let protocol_version = result
            .get("protocolVersion")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AgentFlowError::Custom(
                    "MCP initialize response missing protocolVersion".to_string(),
                )
            })?;

        if protocol_version != "2024-11-05" {
            return Err(AgentFlowError::Custom(format!(
                "Unsupported MCP protocol version from server: {protocol_version}"
            )));
        }

        let server_info = result.get("serverInfo").ok_or_else(|| {
            AgentFlowError::Custom("MCP initialize response missing serverInfo".to_string())
        })?;

        self.server_name = server_info
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        self.server_version = server_info
            .get("version")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        self.initialized = true;

        Ok(())
    }

    /// Return all tools exposed by the connected MCP server.
    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>, AgentFlowError> {
        self.ensure_initialized()?;

        let result = self.request("tools/list", json!({})).await?;
        let tools_value = result.get("tools").cloned().ok_or_else(|| {
            AgentFlowError::Custom("MCP tools/list response missing tools array".to_string())
        })?;

        serde_json::from_value::<Vec<McpTool>>(tools_value)
            .map_err(|e| AgentFlowError::Custom(format!("Invalid MCP tools/list payload: {e}")))
    }

    /// Call a named tool with JSON object arguments.
    pub async fn call_tool(
        &mut self,
        name: impl Into<String>,
        arguments: Value,
    ) -> Result<McpCallResult, AgentFlowError> {
        self.ensure_initialized()?;

        let result = self
            .request(
                "tools/call",
                json!({
                    "name": name.into(),
                    "arguments": arguments,
                }),
            )
            .await?;

        serde_json::from_value::<McpCallResult>(result)
            .map_err(|e| AgentFlowError::Custom(format!("Invalid MCP tools/call payload: {e}")))
    }

    /// Return server name captured during `initialize`, if available.
    pub fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }

    /// Return server version captured during `initialize`, if available.
    pub fn server_version(&self) -> Option<&str> {
        self.server_version.as_deref()
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value, AgentFlowError> {
        let id = self.next_id;
        self.next_id += 1;

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let request_line = serde_json::to_string(&request)?;
        self.stdin.write_all(request_line.as_bytes()).await.map_err(|e| {
            AgentFlowError::Custom(format!("Failed to write MCP request '{method}': {e}"))
        })?;
        self.stdin.write_all(b"\n").await.map_err(|e| {
            AgentFlowError::Custom(format!("Failed to terminate MCP request '{method}': {e}"))
        })?;
        self.stdin.flush().await.map_err(|e| {
            AgentFlowError::Custom(format!("Failed to flush MCP request '{method}': {e}"))
        })?;

        let response_line = self.read_next_response_line(method).await?;
        let response: Value = serde_json::from_str(&response_line).map_err(|e| {
            AgentFlowError::Custom(format!(
                "Failed to parse MCP response for '{method}' as JSON: {e}"
            ))
        })?;

        let jsonrpc = response
            .get("jsonrpc")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AgentFlowError::Custom(format!(
                    "MCP response for '{method}' missing jsonrpc field"
                ))
            })?;
        if jsonrpc != "2.0" {
            return Err(AgentFlowError::Custom(format!(
                "Invalid jsonrpc version in MCP response for '{method}': {jsonrpc}"
            )));
        }

        let response_id = response.get("id").ok_or_else(|| {
            AgentFlowError::Custom(format!("MCP response for '{method}' missing id field"))
        })?;
        if response_id != &json!(id) {
            return Err(AgentFlowError::Custom(format!(
                "MCP response id mismatch for '{method}': expected {id}, got {response_id}"
            )));
        }

        if let Some(error) = response.get("error") {
            let code = error.get("code").and_then(Value::as_i64).unwrap_or(-32000);
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Unknown MCP server error");
            let data = error.get("data").cloned();
            let rendered = if let Some(data) = data {
                format!("MCP server error {code}: {message}; data={data}")
            } else {
                format!("MCP server error {code}: {message}")
            };
            return Err(AgentFlowError::NodeFailure(rendered));
        }

        response.get("result").cloned().ok_or_else(|| {
            AgentFlowError::Custom(format!("MCP response for '{method}' missing result field"))
        })
    }

    async fn read_next_response_line(&mut self, method: &str) -> Result<String, AgentFlowError> {
        loop {
            match self.stdout_lines.next_line().await {
                Ok(Some(line)) if !line.trim().is_empty() => return Ok(line),
                Ok(Some(_)) => continue,
                Ok(None) => {
                    let status = self.child.wait().await.map_err(|e| {
                        AgentFlowError::Custom(format!(
                            "MCP server closed stdout unexpectedly during '{method}': {e}"
                        ))
                    })?;
                    return Err(AgentFlowError::Custom(format!(
                        "MCP server closed stdout unexpectedly during '{method}' with status {status}"
                    )));
                }
                Err(e) => {
                    return Err(AgentFlowError::Custom(format!(
                        "Failed reading MCP response for '{method}': {e}"
                    )));
                }
            }
        }
    }

    fn ensure_initialized(&self) -> Result<(), AgentFlowError> {
        if self.initialized {
            Ok(())
        } else {
            Err(AgentFlowError::Custom(
                "McpClient must be initialized before use".to_string(),
            ))
        }
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}
