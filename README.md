# AgentFlow

[![Crates.io](https://img.shields.io/crates/v/agentflow.svg)](https://crates.io/crates/agentflow)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

**AgentFlow** is a provider-agnostic, async-native Rust framework for building LLM agents, RAG pipelines, multi-agent workflows, and agentic orchestration systems.

Bring your own LLM provider (`rig`, `openai`, `anthropic`, etc.) — AgentFlow handles the orchestration.

---

## Features

| Feature | Description |
|---|---|
| `Flow` | Directed graph of nodes with labeled-edge routing and cycle prevention |
| `Agent` | Async decision unit with configurable retry logic |
| `Workflow` | Linear chain of nodes with conditional branching |
| `MultiAgent` | Parallel agent execution with 3 merge strategies |
| `MapReduce` | Batch map + reduce over document collections |
| `Rag` | Retriever → Generator pipeline |
| `StructuredOutput` | Typed JSON output extraction from LLM responses |
| `Store` / `TypedStore` | Type-safe wrappers over the shared store |
| `TypedFlow` | Generic flow over user-defined state structs |
| `ResultNode` | Nodes that return `Result<SharedStore, AgentFlowError>` |
| `AgentFlowError` | Unified error type: `NotFound`, `Timeout`, `NodeFailure`, `ExecutionLimitExceeded` |
| `tracing` | Built-in structured logging via the `tracing` crate |
| `skills` feature | Skill parser, tool nodes, RPI workflow |
| `mcp` feature | MCP server over stdio for Claude Desktop / Cursor |

---

## Installation

```toml
[dependencies]
agentflow = "0.2"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

Optional features:

```toml
# YAML skill files + RPI workflow
agentflow = { version = "0.2", features = ["skills"] }

# MCP server (includes skills)
agentflow = { version = "0.2", features = ["mcp"] }

# Qdrant-backed RAG
agentflow = { version = "0.2", features = ["rag"] }

# Interactive REPL / TUI
agentflow = { version = "0.2", features = ["repl"] }
```

---

## Quick Start

```rust
use agentflow::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    let node_a = create_node(|store: SharedStore| async move {
        store.write().await.insert("result_a".into(), serde_json::json!("done by A"));
        store.write().await.insert("action".into(), serde_json::json!("next"));
        store
    });

    let node_b = create_node(|store: SharedStore| async move {
        let prev = store.read().await
            .get("result_a").and_then(|v| v.as_str()).unwrap_or("").to_string();
        store.write().await.insert("final".into(), serde_json::json!(format!("B got: {}", prev)));
        store
    });

    let mut flow = Flow::new();
    flow.add_node("a", node_a);
    flow.add_node("b", node_b);
    flow.add_edge("a", "next", "b");

    let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
    let result = flow.run(store).await;
    println!("{}", result.read().await["final"]); // B got: done by A
}
```

---

## Core Concepts

### SharedStore

```rust
pub type SharedStore = Arc<RwLock<HashMap<String, serde_json::Value>>>;
```

- All nodes communicate through a single `SharedStore`.
- The `"action"` key is reserved by `Flow` for routing decisions.
- **Always drop the write guard before `.await`** to avoid deadlocks.

```rust
// ❌ deadlock risk
let mut g = store.write().await;
g.insert("key".into(), val);
some_async_fn().await; // lock still held!

// ✅ correct
{
    let mut g = store.write().await;
    g.insert("key".into(), val);
} // guard dropped here
some_async_fn().await;
```

### Nodes

```rust
// Infallible node
let node = create_node(|store: SharedStore| async move {
    store.write().await.insert("key".into(), serde_json::json!("value"));
    store
});

// Fallible node — returns Result
let node = create_result_node(|store: SharedStore| async move {
    if store.read().await.contains_key("bad") {
        return Err(AgentFlowError::NodeFailure("bad input".into()));
    }
    Ok(store)
});
```

---

## Patterns

### Agent

Retry-aware async decision unit.

```rust
let agent = Agent::with_retry(my_node, 3, 500); // 3 retries, 500ms delay
let result = agent.decide_shared(store).await;

// Fallible variant — distinguishes Timeout vs NodeFailure
let agent2 = Agent::new(my_node2);
let result = agent2.decide_result(store, &my_result_node).await?;
```

### Flow

Directed graph with labeled-edge routing and infinite-loop prevention.

```rust
let mut flow = Flow::new().with_max_steps(50);

flow.add_node("planner",   planner_node);
flow.add_node("executor",  executor_node);
flow.add_node("validator", validator_node);

flow.add_edge("planner",   "execute",  "executor");
flow.add_edge("executor",  "validate", "validator");
flow.add_edge("validator", "retry",    "planner"); // loop back

// run() writes "error" key on limit exceeded
let result = flow.run(store).await;

// run_safe() returns Err(ExecutionLimitExceeded) instead
let result = flow.run_safe(store).await?;
```

### Workflow

Linear steps with conditional branching.

```rust
let mut wf = Workflow::new();
wf.add_step("research", research_node);
wf.add_step("write",    write_node);
wf.add_step("review",   review_node);
wf.connect("research", "write");
wf.connect("write",    "review");

let result = wf.execute_shared(store).await;
```

### MultiAgent

Parallel agent execution with configurable merge strategies.

```rust
// Strategy 1: SharedStore (default) — all agents share one store
let mut multi = MultiAgent::new();
multi.add_agent(researcher_node);
multi.add_agent(coder_node);

// Strategy 2: Namespaced — outputs keyed as "agent_0.*", "agent_1.*"
let mut multi = MultiAgent::with_strategy(MergeStrategy::Namespaced);

// Strategy 3: Custom merge function
let mut multi = MultiAgent::with_strategy(MergeStrategy::Custom(my_merge_fn));

let result = multi.run(store).await;
```

### RAG

```rust
let rag = Rag::new(retriever_node, generator_node);
// retriever writes "context" → generator reads "context", writes "response"
let result = rag.call(store).await;
```

### MapReduce

```rust
let mr = MapReduce::new(mapper_node, reducer_node);
let result = mr.run(vec![store1, store2, store3]).await;
```

### Store — typed access helper

```rust
use agentflow::core::store::Store;

let store = Store::new();
store.set_string("name", "Alice").await;
store.set_i64("age", 30).await;
store.set_bool("active", true).await;

let name: Option<String> = store.get_string("name").await;
let age:  Option<i64>    = store.get_i64("age").await;
let name: String         = store.require_string("name").await?; // Err if missing

let shared: SharedStore  = store.into_shared();
```

### TypedFlow — generic state machine

```rust
use agentflow::core::{TypedFlow, TypedStore, create_typed_node};

#[derive(Debug, Clone)]
struct MyState { count: u32 }

let mut flow = TypedFlow::<MyState>::new().with_max_steps(10);

let node = create_typed_node(|s: TypedStore<MyState>| async move {
    s.inner.write().await.count += 1;
    s
});

flow.add_node("inc", node);
flow.add_transition("inc", |state| {
    if state.count < 5 { Some("inc".into()) } else { None }
});

let final_state = flow.run(TypedStore::new(MyState { count: 0 })).await;
// final_state.inner.read().await.count == 5
```

### Error Handling

```rust
use agentflow::core::error::AgentFlowError;

let node = create_result_node(|store: SharedStore| async move {
    store.read().await
        .get("input").cloned()
        .ok_or_else(|| AgentFlowError::NotFound("input key missing".into()))?;
    Ok(store)
});

match flow.run_safe(store).await {
    Ok(store) => { /* success */ }
    Err(AgentFlowError::ExecutionLimitExceeded(msg)) => eprintln!("Loop: {}", msg),
    Err(e) => eprintln!("Error: {}", e),
}
```

---

## Examples

All examples live in [`examples/`](./examples/). Run with `cargo run --example <name>`.

| Example | Description |
|---|---|
| `agent` | Single LLM agent with retry |
| `async-agent` | Two agents running concurrently |
| `workflow` | Multi-step workflow with human-in-the-loop |
| `rag` | RAG pipeline (retrieve → generate) |
| `multi-agent` | Parallel agents, shared store |
| `mapreduce` | Batch map + reduce over documents |
| `orchestrator-multi-agent` | Orchestrator coordinating multi-role agents |
| `orchestrator-with-tools` | Orchestrator with tool-calling nodes |
| `structured-output` | Typed JSON extraction from LLM |
| `error-handling` | `ResultNode` + `AgentFlowError` patterns |
| `react` | ReAct (Reason + Act) loop |
| `reflection` | Self-critique + reflection loop |
| `plan-and-execute` | Planner → executor pattern |
| `routing` | Conditional routing between nodes |
| `repl` | Interactive REPL agent loop |
| `typed-flow` | `TypedFlow` over a custom state struct |
| `dynamic-orchestrator` | TOML-configured agent registry, runtime orchestration |
| `rpi` | Research → Plan → Implement → Verify loop |
| `rust-agentic-skills` | YAML skill files + RPI workflow (`--features skills`) |
| `document-processing` | Document pipeline with skill nodes (`--features skills`) |

### Environment setup

```bash
export OPENAI_API_KEY=sk-...
export GEMINI_API_KEY=...   # if using Gemini
```

Or use a `.env` file — examples call `dotenvy::dotenv().ok()` at startup.

---

## Architecture

```
agentflow/
├── core/
│   ├── node.rs         Node trait, SimpleNode, ResultNode, create_node, create_result_node
│   ├── flow.rs         Flow — labeled-edge graph, max_steps, run / run_safe
│   ├── store.rs        Store — typed helper over SharedStore
│   ├── typed_store.rs  TypedStore<T> — generic state wrapper
│   ├── typed_flow.rs   TypedFlow<T> — generic state machine
│   ├── batch.rs        Batch, ParallelBatch
│   └── error.rs        AgentFlowError
├── patterns/
│   ├── agent.rs        Agent — retry, decide_shared, decide_result
│   ├── workflow.rs     Workflow — linear steps, execute_shared
│   ├── multi_agent.rs  MultiAgent — SharedStore / Namespaced / Custom
│   ├── rag.rs          Rag — retriever + generator
│   ├── mapreduce.rs    MapReduce
│   ├── structured_output.rs
│   └── rpi.rs          RpiWorkflow
├── utils/
│   └── tool.rs         create_tool_node — shell commands as nodes
├── skills/             (feature: skills) YAML skill parser
└── mcp/                (feature: mcp) MCP stdio server
```

---

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
