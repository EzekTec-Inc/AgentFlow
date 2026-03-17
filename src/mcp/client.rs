use crate::core::error::AgentFlowError;
use rmcp::{
    model::{
        CallToolRequestParam, ClientInfo, Content, ReadResourceRequestParam, Resource,
        ResourceContents, Tool,
    },
    service::RunningService,
    transport::child_process::TokioChildProcess,
    RoleClient, ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;

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
    /// Whether the remote server marked the result as an error.
    #[serde(rename = "isError")]
    pub is_error: Option<bool>,
}

/// Minimal MCP resource descriptor returned by `resources/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    /// Resource URI.
    pub uri: String,
    /// Human-readable resource name.
    pub name: String,
    /// Optional resource description.
    pub description: Option<String>,
    /// Optional MIME type.
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    /// Optional byte size when provided by the server.
    pub size: Option<u32>,
}

/// Minimal MCP `resources/read` result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpReadResourceResult {
    /// Structured resource content entries returned by the server.
    pub contents: Vec<McpResourceContents>,
}

/// Owned MCP resource content entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpResourceContents {
    /// Text resource content.
    #[serde(rename_all = "camelCase")]
    Text {
        /// Resource URI.
        uri: String,
        /// Optional MIME type.
        mime_type: Option<String>,
        /// Text payload.
        text: String,
    },
    /// Base64-encoded blob resource content.
    #[serde(rename_all = "camelCase")]
    Blob {
        /// Resource URI.
        uri: String,
        /// Optional MIME type.
        mime_type: Option<String>,
        /// Blob payload.
        blob: String,
    },
}

/// rmcp-backed MCP client supporting stdio child processes and streamable HTTP.
pub struct McpClient {
    service: RunningService<RoleClient, ClientInfo>,
    server_name: Option<String>,
    server_version: Option<String>,
}

impl McpClient {
    /// Spawn a stdio MCP server process and complete the rmcp initialization handshake.
    pub async fn spawn_stdio(
        command: Command,
        options: McpClientOptions,
    ) -> Result<Self, AgentFlowError> {
        let transport = TokioChildProcess::new(command).map_err(|e| {
            AgentFlowError::Custom(format!("Failed to prepare MCP stdio transport: {e}"))
        })?;

        let service = client_info(options).serve(transport).await.map_err(|e| {
            AgentFlowError::Custom(format!("Failed to initialize MCP stdio client: {e}"))
        })?;

        Ok(Self::from_service(service))
    }

    /// Return all tools exposed by the connected MCP server.
    pub async fn list_tools(&self) -> Result<Vec<McpTool>, AgentFlowError> {
        let tools = self
            .service
            .list_all_tools()
            .await
            .map_err(|e| AgentFlowError::Custom(format!("MCP tools/list failed: {e}")))?;

        tools.into_iter().map(Self::convert_tool).collect()
    }

    /// Return all resources exposed by the connected MCP server.
    pub async fn list_resources(&self) -> Result<Vec<McpResource>, AgentFlowError> {
        let resources = self
            .service
            .list_all_resources()
            .await
            .map_err(|e| AgentFlowError::Custom(format!("MCP resources/list failed: {e}")))?;

        resources.into_iter().map(Self::convert_resource).collect()
    }

    /// Read a named resource by URI.
    pub async fn read_resource(
        &self,
        uri: impl Into<String>,
    ) -> Result<McpReadResourceResult, AgentFlowError> {
        let result = self
            .service
            .read_resource(ReadResourceRequestParam { uri: uri.into() })
            .await
            .map_err(|e| AgentFlowError::Custom(format!("MCP resources/read failed: {e}")))?;

        Ok(McpReadResourceResult {
            contents: result
                .contents
                .into_iter()
                .map(Self::convert_resource_contents)
                .collect(),
        })
    }

    /// Call a named tool with JSON object arguments.
    pub async fn call_tool(
        &self,
        name: impl Into<String>,
        arguments: Value,
    ) -> Result<McpCallResult, AgentFlowError> {
        let arguments = match arguments {
            Value::Object(map) => Some(map),
            Value::Null => None,
            other => {
                return Err(AgentFlowError::Custom(format!(
                    "MCP tool arguments must be a JSON object or null, got {other}"
                )))
            }
        };

        let result = self
            .service
            .call_tool(CallToolRequestParam {
                name: name.into().into(),
                arguments,
            })
            .await
            .map_err(|e| AgentFlowError::Custom(format!("MCP tools/call failed: {e}")))?;

        Ok(McpCallResult {
            content: result.content.into_iter().map(content_to_json).collect(),
            is_error: result.is_error,
        })
    }

    /// Return server name captured during initialize, if available.
    pub fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }

    /// Return server version captured during initialize, if available.
    pub fn server_version(&self) -> Option<&str> {
        self.server_version.as_deref()
    }

    /// Cancel the underlying rmcp connection.
    pub async fn shutdown(self) -> Result<(), AgentFlowError> {
        self.service
            .cancel()
            .await
            .map(|_| ())
            .map_err(|e| AgentFlowError::Custom(format!("Failed to shut down MCP client: {e}")))
    }

    fn from_service(service: RunningService<RoleClient, ClientInfo>) -> Self {
        let peer_info = service.peer_info().cloned();
        Self {
            server_name: peer_info.as_ref().map(|info| info.server_info.name.clone()),
            server_version: peer_info
                .as_ref()
                .map(|info| info.server_info.version.clone()),
            service,
        }
    }

    fn convert_tool(tool: Tool) -> Result<McpTool, AgentFlowError> {
        Ok(McpTool {
            name: tool.name.into_owned(),
            description: tool.description.map(|d| d.into_owned()).unwrap_or_default(),
            input_schema: serde_json::to_value(&*tool.input_schema).map_err(|e| {
                AgentFlowError::Custom(format!("Failed to serialize MCP tool schema: {e}"))
            })?,
        })
    }

    fn convert_resource(resource: Resource) -> Result<McpResource, AgentFlowError> {
        let raw = serde_json::to_value(&resource)
            .map_err(|e| AgentFlowError::Custom(format!("Failed to serialize MCP resource: {e}")))?;

        serde_json::from_value(raw)
            .map_err(|e| AgentFlowError::Custom(format!("Failed to deserialize MCP resource: {e}")))
    }

    fn convert_resource_contents(contents: ResourceContents) -> McpResourceContents {
        match contents {
            ResourceContents::TextResourceContents {
                uri,
                mime_type,
                text,
            } => McpResourceContents::Text {
                uri,
                mime_type,
                text,
            },
            ResourceContents::BlobResourceContents {
                uri,
                mime_type,
                blob,
            } => McpResourceContents::Blob {
                uri,
                mime_type,
                blob,
            },
        }
    }
}

fn client_info(options: McpClientOptions) -> ClientInfo {
    ClientInfo {
        client_info: rmcp::model::Implementation {
            name: options.client_name,
            version: options.client_version,
        },
        ..ClientInfo::default()
    }
}

fn content_to_json(content: Content) -> Value {
    serde_json::to_value(content).unwrap_or_else(|_| Value::Null)
}
