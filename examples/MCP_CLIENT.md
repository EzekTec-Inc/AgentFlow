# MCP Client Tutorial

## What this example is for

This example demonstrates how to create a Model Context Protocol (MCP) client in AgentFlow. Specifically, it spawns an MCP Server process (`mcp_server`), connects to it via stdio, discovers its available tools, and orchestrates an LLM workflow using a typed state machine (`TypedFlow`).

**Primary AgentFlow pattern:** `MCP Client + TypedFlow Orchestrator`  
**Why you would use it:** To give your LLM agents the ability to dynamically discover and execute external tools provided by any MCP-compliant server (e.g. Claude Desktop tools, remote databases, specialized scripts), while managing the overall state securely via `TypedFlow`.

## How it works

The core of this example is the integration between an MCP Client and a strongly typed AgentFlow state machine (`TypedFlow<AppState>`).
1. **Spawn MCP Server**: The client spawns `mcp_server` as a subprocess and connects its standard I/O channels.
2. **Discover Tools**: The client queries the server for its list of available tools.
3. **Execute Flow**: An LLM agent (powered by `rig`) reads its state, calls the MCP tools using `client.call_tool()`, parses the JSON output, and updates the `AppState` iteratively.

### Step-by-Step Code Walkthrough

First, we spawn the MCP server as a subprocess and connect to it over stdio.

```rust
let client = McpClient::spawn_stdio(
    tokio::process::Command::new("path/to/mcp_server"),
    McpClientOptions {
        client_name: "agentflow-mcp-client".into(),
        client_version: "1.0".into(),
    },
).await?;

// List the tools the server is exposing
let mcp_tools = client.list_tools().await?;
info!("Discovered {} MCP tools", mcp_tools.len());
```

Next, we wrap the client in an `Arc<Mutex<McpClient>>` so it can be safely shared across our async flow nodes. Inside a typed node, we can lock the client and execute an MCP tool (like `crawl_goa_url` defined in the server example).

```rust
let crawl_node = create_typed_node(move |store: TypedStore<AppState>| {
    let mcp_client_crawl = Arc::clone(&mcp_client);
    async move {
        let mut state = store.inner.write().await;
        
        // Execute the MCP tool remotely on the server
        let crawl_result = {
            let mut client = mcp_client_crawl.lock().await;
            client.call_tool("crawl_goa_url", json!({ "url": "https://example.com" })).await
        };

        // Parse and handle the result
        if let Ok(result) = crawl_result {
            state.artifacts.push(CrawlArtifact { /* ... */ });
            state.state = StoreState::Crawled;
        }

        store.clone()
    }
});
```

Finally, we construct a strongly typed state machine (`TypedFlow`) and run it to completion. The orchestrator will automatically loop until the state hits the `Complete` or `Failed` action.

```rust
let mut flow = TypedFlow::<AppState>::new().with_max_steps(10);
flow.add_node(Action::CrawlGoADesignSystem, crawl_node);

// Initialize state and start the flow
let initial_store = TypedStore::new(initial_state);
let final_store = flow.run(initial_store).await;
```

## How to run

Because this client spawns the `mcp_server` binary as a subprocess, you must ensure the server is built first.

```bash
cargo build --example mcp_server
cargo run --example mcp_client
```