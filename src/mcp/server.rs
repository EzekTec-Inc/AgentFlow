use crate::core::error::AgentFlowError;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

/// Minimalist MCP (Model Context Protocol) Server for AgentFlow.
/// This reads JSON-RPC requests from stdin, processes them, and writes
/// JSON-RPC responses to stdout.
pub struct McpServer {
    name: String,
    version: String,
}

impl McpServer {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }

    /// Runs the server loop, reading from stdin and writing to stdout.
    pub async fn run(&self) -> Result<(), AgentFlowError> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let mut handle = stdin.lock();
        let mut buffer = String::new();

        loop {
            buffer.clear();
            let bytes_read = handle.read_line(&mut buffer)?;
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
                            // Currently returning an empty list of tools.
                            // In a real implementation, this would list the Skills/Tools available.
                            json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "tools": []
                                }
                            })
                        }
                        "tools/call" => {
                            json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": {
                                    "code": -32601,
                                    "message": "Tool execution not yet fully wired to AgentFlow"
                                }
                            })
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

                    let response_str = serde_json::to_string(&response)?;
                    writeln!(stdout, "{}", response_str)?;
                    stdout.flush()?;
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
                let response_str = serde_json::to_string(&response)?;
                writeln!(stdout, "{}", response_str)?;
                stdout.flush()?;
            }
        }

        Ok(())
    }
}
