# AgentFlow MCP Server Example

The Model Context Protocol (MCP) is a standardized JSON-RPC protocol over standard input/output (stdio) that allows AI agents and orchestrators to expose and consume external tools in a unified manner.

AgentFlow provides built-in support for MCP via the `McpServer` and `skills` Cargo features. The framework maps internal `SkillTool` objects directly to MCP-compatible tool definitions, allowing seamless execution.

This example demonstrates how to spin up an AgentFlow MCP Server that exposes custom tools to external MCP clients (like Claude Desktop or another AgentFlow instance acting as a client).

## Prerequisites
To run this example, ensure you have enabled the `mcp` and `skills` features in Cargo. Since the example sends output via `stdout` and logs via `stderr`, do not pipe `stderr` back into `stdout` when running.

```bash
cargo run --example mcp-server --features="mcp skills"
```

## How It Works
1. **Define a Skill**: The example creates a mock `Skill` containing `SkillTool`s representing shell commands (`echo` and `date`).
2. **Initialize the Server**: `McpServer::new("AgentFlow_System_MCP", "0.1.0")` configures the server metadata.
3. **Register Tools**: Using `.register_skill()`, the server parses the skills and exposes them via the MCP `tools/list` protocol.
4. **Execution**: The `.run().await` method loops over `stdin` to read JSON-RPC payloads, executing `tools/call` requests locally and returning the output through `stdout`.

## Testing the Server Manually
Once the server is running, type raw JSON-RPC messages into the terminal and hit Enter:

### 1. Initialize
```json
{"jsonrpc": "2.0", "id": 1, "method": "initialize"}
```

### 2. List Available Tools
```json
{"jsonrpc": "2.0", "id": 2, "method": "tools/list"}
```

### 3. Call a Tool
Execute the registered `date_tool`:
```json
{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "date_tool", "arguments": {}}}
```

Execute the registered `echo_tool`:
```json
{"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "echo_tool", "arguments": {}}}
```

## Integration with External Clients (e.g. Claude Desktop)
To use your compiled AgentFlow server in external clients, you provide the path to your target executable. Add the following to your MCP client configuration (like `claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "agentflow_mcp": {
      "command": "/path/to/your/compiled/target/debug/examples/mcp-server",
      "args": []
    }
  }
}
```
