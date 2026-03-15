/// MCP stdio client implementation.
pub mod client;

/// MCP server builder and definitions.
pub mod server;

pub use client::{McpCallResult, McpClient, McpClientOptions, McpTool};
pub use server::McpServer;
