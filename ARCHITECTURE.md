# AgentFlow — Architecture Reference

> **Version:** 0.2.0  
> **Last updated:** 2026-03-10

---

## Table of Contents

1. [Design Philosophy](#design-philosophy)
2. [Crate Layout](#crate-layout)
3. [Core Primitives](#core-primitives)
   - [SharedStore](#sharedstore)
   - [Node & NodeResult](#node--noderesult)
   - [Flow](#flow)
   - [TypedStore & TypedFlow](#typedstore--typedflow)
   - [Store (ergonomic wrapper)](#store-ergonomic-wrapper)
   - [Batch & ParallelBatch](#batch--parallelbatch)
   - [AgentFlowError](#agentflowerror)
4. [Patterns](#patterns)
   - [Agent](#agent)
   - [Workflow](#workflow)
   - [MultiAgent](#multiagent)
   - [MapReduce](#mapreduce)
   - [StructuredOutput](#structuredoutput)
   - [Rag](#rag)
5. [Routing Model](#routing-model)
6. [Feature Flags](#feature-flags)
7. [Concurrency & Safety Rules](#concurrency--safety-rules)
8. [How Pieces Snap Together](#how-pieces-snap-together)

---

## Design Philosophy

| Principle | What it means in practice |
|-----------|--------------------------|
| **Bring your own LLM** | AgentFlow is orchestration-only. LLM calls live inside your nodes (`rig-core`, `async-openai`, any HTTP client). |
| **Graph + Shared Store** | Every pattern is a directed graph of `Node`s that communicate through a `SharedStore`. |
| **Composable** | Primitives snap together. A `Flow` can contain a `Workflow`; a `MultiAgent` can contain `Agent`s. |
| **Async-first** | All execution is `async`/`await`. Concurrent patterns use Tokio tasks under the hood. |
| **Type-safe escape hatch** | `SharedStore` (flexible JSON map) and `TypedStore<T>` (compile-time struct) coexist so you choose the right tool. |

---

## Crate Layout

```
src/
├── lib.rs                  # Public API surface + prelude
├── core/
│   ├── node.rs             # Node, NodeResult, SharedStore, factory fns
│   ├── flow.rs             # Flow — the graph executor
│   ├── store.rs            # Store — ergonomic typed wrapper over SharedStore
│   ├── typed_store.rs      # TypedStore<T> — compile-time typed state
│   ├── typed_flow.rs       # TypedFlow<T> — typed graph executor
│   ├── batch.rs            # Batch, ParallelBatch
│   ├── error.rs            # AgentFlowError
│   └── mod.rs
├── patterns/
│   ├── agent.rs            # Agent — retry + decision loop
│   ├── workflow.rs         # Workflow — sequential pipeline
│   ├── multi_agent.rs      # MultiAgent — concurrent fan-out
│   ├── mapreduce.rs        # MapReduce — scatter/gather
│   ├── structured_output.rs# StructuredOutput — schema-validated output
│   ├── rag.rs              # Rag — retrieval-augmented generation
│   ├── batchflow.rs        # BatchFlow — batch processing over flows
│   ├── rpi.rs              # RpiWorkflow (skills feature)
│   └── mod.rs
├── utils/
│   └── tool.rs             # Shell tool nodes
├── skills/                 # (feature: skills) YAML skill parser
└── mcp/                    # (feature: mcp) MCP stdio server
```

---

## Core Primitives

### SharedStore

```rust
pub type SharedStore = Arc<tokio::sync::RwLock<HashMap<String, serde_json::Value>>>;
```

- The central data bus. Every node reads from and writes to it.
- Cloning a `SharedStore` shares the **same** underlying data (cheap `Arc` clone).
- All values are `serde_json::Value` — flexible but runtime-cast.
- **Reserved key:** `"action"` — used exclusively by `Flow` for routing. Do not use it for application data.

### Node & NodeResult

```rust
// Infallible node
pub trait Node<I, O>: Send + Sync + DynClone { ... }
pub type SimpleNode = Box<dyn Node<SharedStore, SharedStore>>;

// Fallible node
pub trait NodeResult<I, O>: Send + Sync + DynClone { ... }
pub type ResultNode = Box<dyn NodeResult<SharedStore, SharedStore>>;
```

**Factory functions:**

| Function | Returns | Use when |
|----------|---------|----------|
| `create_node(closure)` | `SimpleNode` | Node cannot fail |
| `create_result_node(closure)` | `ResultNode` | Node may return `AgentFlowError` |
| `create_retry_node(prep, exec, post, retries, wait, fallback)` | `SimpleNode` | Need prep/exec/post separation with built-in retry |
| `create_batch_node(closure)` | `SimpleNode` | Processing a list of items |

### Flow

`Flow` is the directed-graph executor. It routes between nodes by reading the `"action"` key from the store after each node executes.

```
node_a ──"next"──► node_b ──"done"──► node_c
                        └──"retry"──► node_a
```

**Key methods:**

| Method | Description |
|--------|-------------|
| `Flow::new()` | Create an empty flow |
| `.add_node(name, node)` | Register a node |
| `.add_edge(from, action, to)` | Add a directed edge |
| `.with_start(name)` | Override the start node (default: first added) |
| `.with_max_steps(n)` | Cap total execution steps (cycle prevention) |
| `.run(store)` | Execute; returns store (silently stops at limit) |
| `.run_safe(store)` | Execute; returns `Result<SharedStore, AgentFlowError>` |

**Routing contract:**
1. Node writes `"action"` key into the store (e.g. `"next"`, `"retry"`, `"done"`).
2. `Flow` reads and removes `"action"`, looks up the matching outgoing edge, advances.
3. If no `"action"` key is written, or no matching edge exists, execution halts.

### TypedStore & TypedFlow

For strict state machines where you want compile-time guarantees:

```rust
#[derive(Clone)]
struct MyState { step: u32, result: String }

let store = TypedStore::new(MyState { step: 0, result: String::new() });

let mut flow = TypedFlow::new();
flow.add_node("a", create_typed_node(|store: TypedStore<MyState>| async move {
    store.inner.write().await.step += 1;
    store
}));
flow.add_transition("a", |state| {
    if state.step < 3 { Some("a".into()) } else { None }
});
```

| | `SharedStore` / `Flow` | `TypedStore<T>` / `TypedFlow<T>` |
|---|---|---|
| Key access | Runtime JSON cast | Compile-time struct fields |
| Routing | `"action"` key in store | Closure over `&T` |
| Flexibility | High | Low — fixed struct shape |
| Best for | Dynamic / ad-hoc pipelines | Strict state machines |

### Store (ergonomic wrapper)

`Store` wraps a `SharedStore` with typed `get<T>`, `set`, and `require` helpers — eliminating manual `serde_json` casts in node bodies:

```rust
// Wrap an existing SharedStore
let mut s = Store::from_shared(store.clone());
s.set("count", 42u32);
let n: u32 = s.require("count").await?;

// Or start fresh
let mut s = Store::new();
s.set("key", "value");
```

### Batch & ParallelBatch

| Type | Behaviour |
|------|-----------|
| `Batch` | Runs a `SimpleNode` over each item in a `Vec<SharedStore>` **sequentially** |
| `ParallelBatch` | Runs the same node over all items **concurrently** via `join_all` |

### AgentFlowError

```rust
pub enum AgentFlowError {
    NotFound(String),           // Missing key or resource
    Timeout(String),            // Transient — retried by Agent
    NodeFailure(String),        // Fatal — retries skipped
    ExecutionLimitExceeded(String), // Flow max_steps hit
    TypeMismatch(String),       // Wrong value type in store
    Custom(String),             // Catch-all
}
```

Implements `std::error::Error`, `Display`, `From<std::io::Error>`, `From<serde_json::Error>`.

---

## Patterns

### Agent

An autonomous decision-making unit wrapping any `Node` with:
- **Retry logic** — configurable `max_retries` and `wait_duration`.
- **Transient vs fatal error classification** — `Timeout` is retried; `NodeFailure` is not.
- **Result-aware variant** — `decide_result` for `ResultNode`-backed agents.

```rust
let agent = Agent::with_retry(my_node, 3, Duration::from_millis(500));

let result = agent.decide_shared(store).await;         // infallible
let result = agent.decide_result(store, &r_node).await; // Result<SharedStore, AgentFlowError>
```

### Workflow

A sequential pipeline of nodes. Each node executes in order; the store threads through all of them.

```rust
let mut wf = Workflow::new();
wf.add_step(node_a);
wf.add_step(node_b);
let result = wf.execute_shared(store).await;
```

### MultiAgent

Runs multiple agents **concurrently** using `join_all` and merges results.

| Strategy | Isolation | Output keys |
|----------|-----------|-------------|
| `MergeStrategy::SharedStore` | None — shared `Arc` | As written by each agent |
| `MergeStrategy::Namespaced` | Snapshot per agent | `"agent_0.key"`, `"agent_1.key"`, … |
| `MergeStrategy::Custom(fn)` | Snapshot per agent | Determined by your merge function |

### MapReduce

Scatter-gather over a dataset:
1. **Map** — runs a node over each item, producing per-item results.
2. **Reduce** — aggregates all results into a single store.

### StructuredOutput

Validates and extracts a typed `T: DeserializeOwned` from the store under a given key, returning `Result<T, AgentFlowError>`.

### Rag

Retrieval-augmented generation bridge. Connects a vector store (Qdrant, behind the `rag` feature flag) to a query node, injecting retrieved context into the store before the LLM call.

---

## Routing Model

```
┌──────────┐   write "action" = "next"   ┌──────────┐
│  Node A  │ ──────────────────────────► │  Flow    │ ──► Node B
└──────────┘                             │  reads   │
                                         │ "action" │
                                         └──────────┘
```

**Rules:**
1. `"action"` is the **only** reserved routing key. Never use it for application data.
2. Nodes that want to halt the flow simply do **not** write `"action"`.
3. `Flow` removes `"action"` from the store after reading it — nodes never see a stale value.
4. Unrecognised `"action"` values (no matching edge) also halt execution silently.

---

## Feature Flags

| Flag | Enables | Extra deps |
|------|---------|-----------|
| *(default)* | `core`, `patterns`, `utils` | `tokio`, `serde_json`, `futures`, `tracing` |
| `skills` | YAML skill parser, `RpiWorkflow` | `serde_yaml` |
| `mcp` | MCP stdio server (implies `skills`) | `rmcp` |
| `rag` | Qdrant-backed retrieval | `qdrant-client`, `fastembed` |
| `repl` | Interactive REPL / TUI | `inquire` |

Activate in `Cargo.toml`:
```toml
[dependencies]
agentflow = { version = "0.2", features = ["skills", "repl"] }
```

---

## Concurrency & Safety Rules

1. **Never hold a write guard across an `.await` point** — this deadlocks under Tokio's cooperative scheduler.
   ```rust
   // ✅ correct
   { store.write().await.insert("k".into(), json!("v")); }
   some_async_fn().await;

   // ❌ deadlock
   let mut g = store.write().await;
   some_async_fn().await;  // g still held!
   g.insert(...);
   ```
2. **Use distinct output keys in `MultiAgent::SharedStore` strategy** — all agents share one `Arc`; concurrent writes to the same key produce a last-write-wins race.
3. **Prefer `Namespaced` or `Custom` strategies** when agents produce overlapping keys.
4. **`Flow::with_max_steps`** — always set this in production to prevent runaway loops.

---

## How Pieces Snap Together

```
User Request
     │
     ▼
  Flow (graph executor)
     │
     ├──► SimpleNode (create_node)         ← lightweight, single responsibility
     ├──► Agent (Node + retry)             ← for LLM calls that may fail/retry
     ├──► Workflow (sequential pipeline)   ← ordered steps with shared context
     ├──► MultiAgent (concurrent fan-out)  ← parallel specialised agents
     └──► MapReduce (scatter/gather)       ← batch processing
               │
               ▼
          SharedStore ◄──────────────────── all nodes read/write here
          (or TypedStore<T>)
```

A `Flow` is the top-level orchestrator. Inside each node you can embed any pattern — a node can itself run a `Workflow`, invoke a `MultiAgent`, or call another `Flow`. There is no depth limit.
