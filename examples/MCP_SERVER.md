# MCP Server Tutorial

## What this example is for

This example demonstrates how to create a Model Context Protocol (MCP) server using AgentFlow. It shows how to use the `Skill` parser to define tools from a YAML/Markdown string and expose them via standard stdio for any MCP-compliant client.

**Primary AgentFlow pattern:** `MCP Server`  
**Why you would use it:** When you want to expose a set of command-line tools, HTTP requests, or local scripts to an LLM agent (like Claude Desktop or another AgentFlow client) using the standardized Model Context Protocol.

## How it works

The code is incredibly minimal. It defines an MCP Server named `GoA_Research_Server` and registers a "Skill" into it. A Skill in AgentFlow is a declarative document that defines tools. In this case, it defines two tools:
1. `crawl_goa_url`: Runs a `curl` command to fetch a URL.
2. `render_report_summary`: Runs a `printf` command to format a string.

Once the skill is parsed and registered, `server.run().await?` starts listening on `stdin`/`stdout` for JSON-RPC messages following the MCP protocol.

### Step-by-Step Code Walkthrough

First, we define the tools using AgentFlow's declarative Skill format. This is embedded directly in the code as a raw string for demonstration, but it could easily be loaded from a `.md` or `.yaml` file.

```rust
let skill_content = r#"---
name: GoAResearchTools
description: Tools for crawling the Government of Alberta design system.
version: 1.0.0
tools:
  - name: crawl_goa_url
    description: Fetches content from a URL
    command: curl
    args: ["-sL", "{{url}}"]
  - name: render_report_summary
    description: Echoes a report summary title.
    command: printf
    args: ["Report ready: %s\n", "{{title}}"]
---
"#;

// Parse the raw text into an executable Skill struct
let skill = Skill::parse(skill_content)?;
```

Next, we instantiate the `McpServer`, pass the skill to it via the builder pattern, and start the server.

```rust
// Create the server, register the tools, and start listening on stdio
let server = McpServer::new("GoA_Research_Server", "0.2.0")
    .register_skill(skill);

server.run().await?;
```

Because this server communicates over `stdio`, it is meant to be executed as a subprocess by an MCP client, not run interactively in a terminal.

## How to run

To test this server, you would typically configure an MCP client (like Claude Desktop or the AgentFlow MCP Client) to run this binary. However, you can compile and run it to verify it builds:

```bash
cargo build --example mcp_server
```

*(Note: Running it directly in the terminal will appear to hang because it is waiting for JSON-RPC payloads on stdin).*