# Implementation Plan: AgentFlow Mitigation Strategies

This plan outlines the step-by-step implementation of the mitigation strategies identified in the `IMPACT_ANALYSIS.md` document, prioritized by risk and effort.

## Phase 1: Critical Fixes (Week 1)
**Focus:** Resolving the `Arc::try_unwrap` blocking anti-pattern and preventing infinite loops in Flow execution.

### Step 1.1: Fix Arc::try_unwrap in Core Patterns
* **Objective:** Remove blocking runtime calls while maintaining backward compatibility.
* **Tasks:**
  1. Modify `Agent<N>`:
     - Introduce `pub async fn decide_shared(&self, input: SharedStore) -> SharedStore` containing the core logic without `Arc::try_unwrap`.
     - Refactor `pub async fn decide(&self, input: HashMap) -> HashMap` to wrap `decide_shared`, handling the extraction safely.
  2. Modify `Workflow`:
     - Introduce `pub async fn execute_shared(&self, mut store: SharedStore) -> SharedStore`.
     - Refactor `pub async fn execute(&self, mut store: HashMap) -> HashMap` to wrap `execute_shared`.
  3. Modify `MultiAgent`:
     - Ensure all merge strategies (`run_shared`, `run_namespaced`, `run_custom`) return `SharedStore` without unwrapping.

### Step 1.2: Implement Flow Step Limiting (Cycle Prevention)
* **Objective:** Prevent infinite loops without the overhead of O(n²) graph analysis.
* **Tasks:**
  1. Update `Flow` struct:
     - Add `max_steps: Option<usize>` field.
     - Add `pub fn with_max_steps(mut self, limit: usize) -> Self` builder method.
  2. Modify `Flow::run`:
     - Implement a step counter in the `while let Some(node) = ...` loop.
     - If `steps >= max_steps`, break the loop and optionally inject an error into the store.
     - Add a `run_safe` method that returns a `Result<SharedStore, AgentFlowError>` for strict checking, while keeping `run` backward-compatible.

### Step 1.3: Update Phase 1 Examples & Tests
* **Objective:** Ensure existing examples work with the new non-blocking APIs.
* **Tasks:**
  1. Update `examples/agent.rs`, `examples/workflow.rs`, and others to use the new `decide_shared()` or continue using `decide()` correctly.
  2. Add a test case for `Flow` to verify `max_steps` successfully terminates an infinite loop (e.g., A -> B -> A).

---

## Phase 2: Robust Error Handling (Week 2-3)
**Focus:** Moving away from magic strings ("error" key) to type-safe Rust error handling.

### Step 2.1: Define AgentFlowError
* **Objective:** Create a unified error type for the crate.
* **Tasks:**
  1. Create `src/core/error.rs`.
  2. Define `pub enum AgentFlowError { NotFound, Timeout, NodeFailure(String), ExecutionLimitExceeded, ... }`.
  3. Implement `std::fmt::Display` and `std::error::Error`.

### Step 2.2: Implement ResultNode API
* **Objective:** Allow nodes to natively return `Result`.
* **Tasks:**
  1. In `src/core/node.rs`, fully flesh out `pub trait NodeResult<I, O>`.
  2. Create a factory function `pub fn create_result_node<F, Fut>(func: F) -> ResultNode`.
  3. Provide adapters to convert a `SimpleNode` into a `ResultNode` and vice versa, allowing gradual migration.

### Step 2.3: Integrate Errors into Patterns
* **Objective:** Update patterns to respect and propagate `NodeResult`.
* **Tasks:**
  1. Update `Agent` retry logic to distinguish between transient errors (e.g., `Timeout`) and fatal errors, rather than simply checking for an `"error"` key.
  2. Update `Flow::run_safe` to halt execution and return an `Err` if a `ResultNode` fails.

### Step 2.4: Documentation and Example Migration
* **Objective:** Teach users how to use the new error handling.
* **Tasks:**
  1. Create a new example `examples/error_handling.rs` demonstrating `ResultNode`.
  2. Update main `README.md` to highlight the new type-safe error boundaries.

---

## Phase 3: Advanced Features (v0.2+ Backlog)
**Focus:** Optional improvements for power users.

### Step 3.1: TypedStore Implementation
* **Objective:** Provide an alternative to `HashMap<String, Value>` for better performance and compile-time guarantees.
* **Tasks:**
  1. Create `src/core/typed_store.rs`.
  2. Define a generic `TypedStore<T>` wrapper.
  3. Implement `Node` traits for generic types to allow strongly-typed inputs and outputs.
  4. Document when to use `HashMap` vs `TypedStore`.

### Step 3.2: Observability & Telemetry
* **Objective:** Improve debugging.
* **Tasks:**
  1. Add `tracing` instrumentation to `Flow::run` and `Agent::decide`.
  2. Log node transitions, execution times, and retry attempts.

---

## Phase 4: Dynamic Orchestrator with Modular Agents

### Step 4.1: TOML-Driven Agent Configuration
* **Timestamp (UTC):** 2026-03-10T02:45:00Z
* **Objective:** Allow agent settings (name, model, provider, preamble, output_key) to be declared
  in a TOML file (`examples/agents.toml`) rather than hard-coded. If the file does not exist at
  runtime, the example creates it with sensible defaults before proceeding.
* **Files to create/modify:**
  - `examples/dynamic_orchestrator.rs` ← new file (only example, no `src/` changes)
  - `examples/agents.toml` ← created at runtime if absent; committed as a default config
* **Exact reason:** Enables non-Rust users to reconfigure agents (model, preamble, output key)
  without recompiling. Keeps all agent identity outside source code, consistent with the
  project's "LLM-agnostic orchestration" philosophy.
* **TOML schema (`examples/agents.toml`):**
  ```toml
  # Dynamic Orchestrator — agent registry configuration.
  # Each [[agent]] entry is one modular agent the orchestrator may spin up.

  [[agent]]
  name        = "researcher"
  provider    = "openai"          # "openai" | "gemini"
  model       = "gpt-4.1-mini"
  preamble    = "You are a concise research assistant."
  output_key  = "research_result"

  [[agent]]
  name        = "coder"
  provider    = "openai"
  model       = "gpt-4.1-mini"
  preamble    = "You are a senior Rust developer."
  output_key  = "code_result"

  [[agent]]
  name        = "reviewer"
  provider    = "openai"
  model       = "gpt-4.1-mini"
  preamble    = "You are a thorough code reviewer."
  output_key  = "review_result"
  ```
* **Runtime behaviour:**
  1. Example checks for `examples/agents.toml` using `std::path::Path::exists()`.
  2. If missing, writes the default TOML content above to disk and prints a notice.
  3. Parses the TOML into `Vec<AgentConfig>` using the `toml` crate. `toml` is NOT currently
     in the lock file — it must be added to `[dev-dependencies]` in `Cargo.toml` (e.g.
     `toml = "0.8"`). This is the only manifest change required.
  4. Builds an `AgentRegistry` (`HashMap<String, AgentFactory>`) from the parsed configs.
  5. Planner LLM receives the list of available agent names and selects a subset + order.
  6. Dispatcher node pops each `AgentSpec` from the plan, looks up the registry, instantiates
     and runs the agent sequentially so each agent can read the previous agent's output.
  7. Aggregator LLM synthesises all `output_key` values into a final report.
* **Previous behaviour:** N/A — new file.
* **New behaviour:** `cargo run --example dynamic-orchestrator` works with zero manual config;
  power users edit `examples/agents.toml` to swap models/providers without recompiling.
* **Rollback:** Delete `examples/dynamic_orchestrator.rs` and `examples/agents.toml`;
  remove `[[example]]` entry from `Cargo.toml` if added.

### Step 4.2: Living Documentation (`DYNAMIC-ORCHESTRATOR.md`)
* **Timestamp (UTC):** 2026-03-10T03:30:00Z
* **Summary:** Created `examples/DYNAMIC-ORCHESTRATOR.md` as the canonical reference
  for how the dynamic orchestrator works.
* **Files modified:** `examples/DYNAMIC-ORCHESTRATOR.md` ← new file
* **Exact reason:** Persists the architecture analysis (boot sequence, flow wiring,
  planner/dispatcher/aggregator behaviour, SharedStore key reference, TOML schema,
  dependency table, rollback instructions) in the repo so it stays alongside the code.
* **Maintenance rule:** This document MUST be updated whenever `examples/dynamic_orchestrator.rs`
  changes in a way that affects observable behaviour.
* **Previous behaviour:** Analysis existed only in conversation history.
* **New behaviour:** Analysis lives in `examples/DYNAMIC-ORCHESTRATOR.md`, versioned in git.
* **Rollback:** `rm examples/DYNAMIC-ORCHESTRATOR.md`

---

## Gap Resolution Phase 1: Fix Duplicate `[[example]]` Entries

* **Timestamp (UTC):** 2026-03-10 23:14 UTC
* **Summary:** Removed duplicate `[[example]]` entries for `plan-and-execute` and `routing` in `Cargo.toml`.
* **Files modified:** `Cargo.toml`
* **Exact reason:** Duplicate entries caused `cargo` to fail parsing the manifest, breaking `cargo run --example` for all examples.
* **Previous behaviour:** `Cargo.toml` contained two `[[example]]` blocks each for `plan-and-execute` and `routing`, causing a cargo manifest error.
* **New behaviour:** Each example has exactly one `[[example]]` entry; `cargo check` passes cleanly.
* **Rollback:** Re-add the duplicate `[[example]]` blocks for `plan-and-execute` and `routing` after line 108 in `Cargo.toml`.

---

## Gap Resolution Phase 2: ARCHITECTURE.md + CONTRIBUTING.md

* **Timestamp (UTC):** 2026-03-10 23:14 UTC
* **Files created:**
  - `ARCHITECTURE.md` — Full architecture reference: design philosophy, crate layout, all core primitives, all patterns, routing model, feature flags, concurrency rules, and a composability diagram.
  - `CONTRIBUTING.md` — Contributor guide: prerequisites, code conventions, error handling rules, adding patterns/examples, PR process, and commit message format.
* **`cargo check --all-features`** — passes cleanly.
* **Rollback:** Delete `ARCHITECTURE.md` and `CONTRIBUTING.md`.

## [2026-03-12T04:50:00Z] Fix routing state leak in Flow execution and clean up test file
- **Summary of change:** Fixed `"action"` state leak in `Flow` graph routing and removed uncompilable test line in `TypedFlow`.
- **Files modified:** 
  - `src/core/flow.rs`
  - `src/core/typed_flow.rs`
- **Exact reason:** 
  1. If a node executed without overwriting `"action"`, it would reuse the previous node's `"action"`, causing unintended routing or cyclic infinite loops. 
  2. A stray `let new_earth...` line without a semicolon caused `cargo clippy` and `cargo test` compilation failures in `src/core/typed_flow.rs`.
- **Previous behavior:** 
  1. `Flow` acquired a read lock to parse `"action"` but left it in the store for the next node.
  2. `test_typed_flow_execution` failed to compile.
- **New behavior:** 
  1. `Flow` acquires a write lock and uses `.remove("action")` so the routing intent is strictly consumed at edge transitions.
  2. Unused `new_earth` variable removed and `cargo test` passes locally.
- **Rollback instructions:**
  1. Revert `src/core/flow.rs`: Change `.write().await.remove("action")` back to `.read().await.get("action")`.
  2. Revert `src/core/typed_flow.rs`: Add `let new_earth = "New Earth <earth-emoji>"` on line 219.

## [2026-03-12T04:50:20Z] Update documentation to reflect correct Flow and TypedFlow behaviors
- **Summary of change:** Updated `README.md` and `ARCHITECTURE.md` to explicitly describe the consumption of the `"action"` routing key and the `max_steps` cycle-prevention features of `TypedFlow`.
- **Files modified:** 
  - `README.md`
  - `ARCHITECTURE.md`
- **Exact reason:** The documentation lacked explicit mention of the state-leak prevention behavior and omitted `TypedFlow`'s infinite-loop guard (`max_steps`) which were implemented during Phase 3 and the recent bugfix.
- **Previous behavior:** Documentation described `Flow` as just "reading" and removing without explicitly noting it prevents leaks by consuming the key under a write lock, and `TypedFlow` documentation examples lacked the `.with_max_steps()` call.
- **New behavior:** Documentation now accurately describes `Flow` extracting and consuming the `"action"` key under a write lock, and explicitly lists `with_max_steps` for `TypedFlow` examples and mentions its telemetry.
- **Rollback instructions:** Revert edits to `README.md` and `ARCHITECTURE.md` by undoing the Git diff for these changes.
## [2026-03-12T21:58:00Z] Refactor README.md with comprehensive module architecture and Mermaid diagrams
- **Summary of change:** Completely restructured `README.md` to introduce a High-Level Architecture section and specific deep-dives for `core`, `patterns`, `skills`, `utils`, and `mcp`, including Mermaid diagrams for each.
- **Files modified:** `README.md`
- **Exact reason:** The previous documentation lacked a cohesive explanation of how the broader ecosystem (`skills`, `mcp`, `utils`) integrated with the `core` and `patterns`, making it difficult for new developers to understand the full framework architecture.
- **Previous behavior:** `README.md` only highlighted `core` and `patterns` visually, leaving `skills`, `utils`, and `mcp` buried in feature flag lists and the architecture tree.
- **New behavior:** `README.md` now opens with a layered architecture diagram and breaks down each of the 5 main modules with dedicated explanations and Mermaid diagrams, while preserving all existing code examples.
- **Rollback instructions:** Revert edits to `README.md` by undoing the Git diff for these changes (`git checkout -- README.md`).

## [2026-03-12T23:20:00Z] Fix syntax error in High-Level Architecture Mermaid diagram
- **Summary of change:** Fixed unescaped text outside of node brackets in the `README.md` Mermaid diagram which caused a "Syntax error in text" rendering issue.
- **Files modified:** `README.md`
- **Exact reason:** The nodes in the Mermaid graph were formatted incorrectly (e.g., `Skills(skills)<br>YAML Definitions` instead of `Skills("skills<br>YAML Definitions")`), causing Markdown parsers to fail at rendering the diagram.
- **Previous behavior:** High-Level Architecture Mermaid diagram failed to render due to syntax errors.
- **New behavior:** High-Level Architecture Mermaid diagram renders correctly with quoted node labels.
- **Rollback instructions:** Revert edits to `README.md` by undoing the Git diff for this commit.

## [2026-03-12T23:30:00Z] Generate developer documentation for all examples
- **Summary of change:** Extracted the developer-targeted code documentation from all 20 `examples/*.rs` files into dedicated `examples/*.md` files, preserving the exact example names. Merged old `DYNAMIC-ORCHESTRATOR` md files into `dynamic_orchestrator.md` and deleted the old ones.
- **Files modified:** 
  - Added 20 new `examples/*.md` files.
  - Deleted `examples/DYNAMIC-ORCHESTRATOR-DOC.md`
  - Deleted `examples/DYNAMIC-ORCHESTRATOR.md`
- **Exact reason:** To provide explicit, developer-targeted documentation detailing the purpose, mechanics, and usage of each example, directly beside the source code, as requested.
- **Previous behavior:** Only a few examples had dedicated Markdown files (with inconsistent naming).
- **New behavior:** Every example has a dedicated, identically-named `*.md` file explaining how it works, how to adapt it, and providing code examples.
- **Rollback instructions:** Revert this commit using `git revert HEAD` to restore the deleted `DYNAMIC-` files and remove all newly generated `*.md` files.

## [2026-03-12T23:45:00Z] Rename all example markdown documentation files to uppercase
- **Summary of change:** Renamed all 20 generated developer documentation files in `examples/` from lowercase to uppercase (e.g., `agent.md` to `AGENT.md`).
- **Files modified:** 
  - `examples/*.md` (Renamed using `git mv`)
- **Exact reason:** Per user request, the documentation files should have uppercase filenames.
- **Previous behavior:** Markdown files were lowercase.
- **New behavior:** Markdown filenames are all uppercase with `.md` extension.
- **Rollback instructions:** Revert this commit using `git revert HEAD` to restore the lowercase filenames.

## [2026-03-12T23:55:00Z] Add specific Mermaid diagrams to all example documentation
- **Summary of change:** Embedded 20 custom Mermaid diagrams into the newly created `examples/*.md` files, demonstrating the specific flow, nodes, tool usage, and AgentFlow architecture used in each implementation.
- **Files modified:** `examples/*.md` (all 20 developer documentation files)
- **Exact reason:** To provide visual representations of how each example is implemented and where the AgentFlow framework steps in, making it easier for developers to understand the architecture at a glance.
- **Previous behavior:** Markdown files contained only text descriptions of the examples.
- **New behavior:** Every example documentation file now contains a `## Implementation Architecture` section with a targeted Mermaid diagram illustrating the flow and AgentFlow components used.
- **Rollback instructions:** Revert this commit using `git revert HEAD` to remove the Mermaid diagrams from the documentation files.

## [2026-03-13T00:00:00Z] Replace mentions of PocketFlow with AgentFlow
- **Summary of change:** Scanned the entire codebase for mentions of `pocketflow` (case-insensitive) and replaced them with `AgentFlow`.
- **Files modified:** 
  - `examples/AGENT.md`
  - `examples/ASYNC_AGENT.md`
  - `examples/agent.rs`
  - `examples/async_agent.rs`
- **Exact reason:** To ensure consistent branding across the codebase per the user's request.
- **Previous behavior:** Several examples still referenced the old name "PocketFlow".
- **New behavior:** All examples now correctly reference "AgentFlow".
- **Rollback instructions:** Revert this commit using `git revert HEAD` to restore the references to PocketFlow.

## [2026-03-13T00:10:00Z] Update DYNAMIC_ORCHESTRATOR.md to accurately capture implementation
- **Summary of change:** Corrected a few discrepancies in `DYNAMIC_ORCHESTRATOR.md` to perfectly match the `dynamic_orchestrator.rs` implementation.
- **Files modified:** 
  - `examples/DYNAMIC_ORCHESTRATOR.md`
- **Exact reason:** The user requested to investigate and ensure the documentation accurately captures the functionalities, processes, and architecture of the Rust source file. 
- **Previous behavior:** Markdown had incorrect model names (`gpt-4.1-mini`), a wrongly typed file extension in references (`DYNAMIC-ORCHESTRATOR.md`), and potentially outdated descriptions.
- **New behavior:** Documentation now correctly references `gpt-4o-mini`, standardises the filename as `DYNAMIC_ORCHESTRATOR.md`, and accurately reflects the `dynamic_orchestrator.rs` functionalities (which were mostly already perfectly described).
- **Rollback instructions:** Revert this commit using `git revert HEAD` to restore the outdated references in the documentation.

## [2026-03-13T00:20:00Z] Fix routing logic and tool output parsing in orchestrator_with_tools.rs
- **Summary of change:** Modified `orchestrator_with_tools.rs` to correctly route from the `tool` node back to the `reasoner` node, and updated the ReAct response parsing to handle cases where the LLM returns both `ACTION:` and `ANSWER:` strings in a single response.
- **Files modified:** 
  - `examples/orchestrator_with_tools.rs`
- **Exact reason:** The user noticed the example was failing to correctly utilize tools. The issue was twofold: `create_tool_node` does not emit an action (defaults to "default"), so the hardcoded `use_tool` edge was never triggered. Second, the LLM sometimes returned both an action and an answer, trapping the agent in an infinite loop.
- **Previous behavior:** The flow would either immediately terminate after tool execution due to a missing edge, or infinitely loop hitting max steps because it couldn't parse the final answer correctly.
- **New behavior:** The flow correctly transitions from the `tool` node back to the `reasoner` node using the `default` edge, and the parser successfully extracts the final `ANSWER:`, breaking the loop and delivering the real tool output.
- **Rollback instructions:** Revert this commit using `git revert HEAD`.

## [2026-03-13T03:45:00Z] Implement 4 code-review recommendations
- **Summary of change:** Added four new capabilities to the framework.
- **Files modified:**
  - `src/core/node.rs` — `StateDiff` struct + `create_diff_node` factory
  - `src/core/parallel.rs` — `ParallelFlow` (fan-out / fan-in)
  - `src/utils/tool.rs` — `ToolRegistry` allowlist + `create_corrective_retry_node`
  - `src/core/mod.rs`, `src/lib.rs` — exports for new types
  - `examples/plan_and_execute.rs` — updated to use `create_corrective_retry_node`
- **Details:**
  1. **`StateDiff` / `create_diff_node`** — Nodes receive a read-only snapshot of the store, run async work with no lock held, then return a `StateDiff` (inserts + removals). The framework applies the diff under one brief write lock. Deadlocks from holding a lock across `.await` are structurally impossible.
  2. **`ToolRegistry`** — Explicit allowlist mapping names to (command, args, timeout). `create_node(name)` returns `Err(NotFound)` for any name not in the list. LLM-generated tool names can never escape the sandbox. `into_arc()` for cheap cross-task sharing.
  3. **`create_corrective_retry_node`** — Replaces blind retry with a self-correction loop: on each failure the error message is written into the store under a configurable key so the next LLM call can read and adjust.
  4. **`ParallelFlow`** — `ParallelFlow::new(branches)` runs N independent `Flow`s concurrently via `futures::future::join_all`. Each branch receives a snapshot clone (full isolation). `with_merge(fn)` for custom merge; default is last-writer-wins union.
- **Previous behavior:** No fan-out, no allowlisted tools, no deadlock-safe diff node, no self-correcting retry.
- **New behavior:** All four capabilities available and exported from `prelude`. All 20 unit + integration tests pass; 0 clippy warnings.
- **Rollback instructions:** Revert this commit using `git revert HEAD`.

## [2026-03-13T10:05:00Z] Update all documentation
- **Summary of change:** Rewrote `README.md`, `ARCHITECTURE.md`, `CONTRIBUTING.md`, and all 20 `examples/*.md` files to reflect the latest framework state including `StateDiff`, `create_diff_node`, `ToolRegistry`, `create_corrective_retry_node`, and `ParallelFlow`.
- **Files modified:** `README.md`, `ARCHITECTURE.md`, `CONTRIBUTING.md`, `PLAN.md`, `examples/*.md` (all 20)
- **Exact reason:** The four new capabilities added in the previous commit were not reflected in any documentation. Additionally, `CONTRIBUTING.md` and example MDs had stale references and missing sections.
- **Previous behavior:** Docs described v0.2.0 without the new primitives.
- **New behavior:** All docs accurately describe the current codebase including the four new capabilities.
- **Rollback instructions:** Revert this commit using `git revert HEAD`.

## Phase 5: MCP Native Framework Demonstrations

### Step 5.1: Rewrite MCP Server and Client Examples
* **Timestamp (UTC):** 2026-03-14T10:00:00Z
* **Summary:** Rewrote `examples/mcp_server.rs` and `examples/mcp_client.rs` to natively use AgentFlow's built-in `McpServer`, `Skill`, and `create_tool_node` primitives.
* **Files modified:** 
  - `examples/mcp_server.rs`
  - `examples/mcp_client.rs`
* **Exact reason:** The previous implementations relied entirely on external crates (`rmcp` and `rig`), bypassing AgentFlow's native capabilities. The rewrite demonstrates zero-code YAML skill parsing, the native asynchronous JSON-RPC stdio server, `Flow` graph orchestration, and built-in `Agent` fault tolerance.
* **Previous behavior:** `mcp_server.rs` used `rmcp` macros and routing. `mcp_client.rs` manually managed child processes and relied heavily on the `rig` LLM framework to execute tools, ignoring the `SharedStore` and graph architecture.
* **New behavior:** `mcp_server.rs` boots a native `McpServer` exposing a parsed `Skill`. `mcp_client.rs` builds a `Flow` that uses `create_tool_node` to interact with the server, wrapping it in an `Agent` for fault tolerance, and cleanly returning results via the `SharedStore`.
* **Rollback:** `git checkout HEAD^ -- examples/mcp_server.rs examples/mcp_client.rs`

---

## Code Review Resolution Plan

**Timestamp:** 2026-03-14T16:28:41Z
**Source:** Deep code review of all `src/` files against compiled codebase (`cargo check` clean).
**Purpose:** Implementation plan for resolving all 17 issues from the Summary Table.

> Pre-verification notes (findings from source inspection before writing this plan):
> - H3 (`Flow` missing `Default`) — **already resolved**: `impl Default for Flow` exists at `flow.rs:250`. Closed.
> - M2 (`ParallelBatch` unbounded concurrency) — **already resolved**: `ParallelBatch` has `concurrency_limit` defaulting to 10 with `with_concurrency_limit` override. Closed.
> - Confirmed open issues: C1, C2, H1, H2, H4, H5, M1, M3, M4, M5, L1, L2, L3, L4, L5.

---

### Phase 1 — Critical (Address First, Correctness-Breaking)

#### [C1] `clone_store_snapshot` clones `Arc`, not data (`src/core/parallel.rs`)

- **Problem:** `clone_store_snapshot` returns `store.clone()`, which clones the `Arc<RwLock<...>>` — all branches share the same underlying `RwLock`. The promised isolation between branches is absent. Branch writes pollute each other and the final merge is non-deterministic.
- **Fix:** Inside `parallel.rs`, replace `store.clone()` in the map closure with a real deep snapshot: acquire a read lock, clone the `HashMap`, then wrap in a new `Arc::new(RwLock::new(...))`. The `clone_store_snapshot` helper must become `async` or its logic must be inlined directly into the `async move` block.
- **Files:** `src/core/parallel.rs`
- **Backward compatibility:** No public API change. `ParallelFlow::run` signature unchanged.
- **Rollback:** Revert `clone_store_snapshot` body to `store.clone()`.

#### [C2] `BatchFlow` uses `or_insert` — subsequent batch params silently dropped (`src/patterns/batchflow.rs`)

- **Problem:** `store.entry(k).or_insert(v)` at line 33 only inserts if the key is absent. Every batch after the first runs with the first batch's params for any key that collides.
- **Fix:** Replace `store.entry(k.clone()).or_insert(v.clone())` with `store.insert(k.clone(), v.clone())`.
- **Files:** `src/patterns/batchflow.rs`
- **Backward compatibility:** Behavior change — intentional bug fix. Any caller relying on "first value wins" was relying on the bug.
- **Rollback:** Revert the single `insert` call back to `entry().or_insert()`.

---

### Phase 2 — High (API Correctness and Consistency)

#### [H1] `MergeStrategy::Custom` uses bare `fn` pointer (`src/patterns/multi_agent.rs`)

- **Problem:** `Custom(fn(Vec<SharedStore>) -> SharedStore)` cannot capture environment. Closures with captures produce opaque compiler errors for users.
- **Fix:** Change the variant to `Custom(Arc<dyn Fn(Vec<SharedStore>) -> SharedStore + Send + Sync>)`. Update `run_custom` at line 158 to accept `Arc<dyn Fn(...)>`. Update the match arm at line 108 to pass `merge_fn.clone()`.
- **Breaking change:** Yes — `MergeStrategy::Custom(my_fn)` call sites must wrap with `Arc::new(...)`. Document in changelog.
- **Files:** `src/patterns/multi_agent.rs`
- **Rollback:** Revert `Custom` variant and `run_custom` signature.

#### [H2] `Agent::decide_result` takes external `node` param instead of `self.node` (`src/patterns/agent.rs`)

- **Problem:** `decide_result` takes `node: &R` as a separate parameter rather than using the wrapped `self.node`. There is no constructor that places a `ResultNode` inside `Agent`, forcing callers to hold a redundant external reference.
- **Fix:** Add a `run_result` method on `Agent<N>` where `N: NodeResult<SharedStore, SharedStore>` that calls `self.node` directly without an external parameter. Do **not** remove or change the existing `decide_result` (backward compatibility preserved).
- **Files:** `src/patterns/agent.rs`
- **Rollback:** Remove the new `run_result` method.

#### [H4] `Store::require_*` return `Result<_, String>` instead of `Result<_, AgentFlowError>` (`src/core/store.rs`)

- **Problem:** `require`, `require_string`, `require_i64`, `require_f64`, `require_bool` return `Result<_, String>`, breaking the uniform error contract. Callers cannot use `?` to propagate into `AgentFlowError`.
- **Fix:** Change return types to `Result<_, AgentFlowError>`. Map the internal error string to the appropriate `AgentFlowError` variant (`MissingKey` or `Custom` — inspect `error.rs` to confirm available variants before implementing).
- **Breaking change:** Yes — return type changes. Callers pattern-matching on `String` must update. Document in changelog.
- **Files:** `src/core/store.rs`
- **Rollback:** Revert return types to `Result<_, String>`.

#### [H5] `RpiWorkflow` start node is order-dependent (`src/patterns/rpi.rs`)

- **Problem:** `Flow::add_node` sets start node to the first node added. `RpiWorkflow::with_research` relies on being called first with no enforcement. Calling `with_plan` first silently sets `"plan"` as start.
- **Fix:** In `RpiWorkflow::with_research`, after `self.flow.add_node("research", node)`, explicitly force the start node to `"research"`. If `Flow` does not expose a `set_start` method, add `pub fn set_start(&mut self, name: &str)` to `Flow` (one-line addition to `flow.rs`).
- **Files:** `src/patterns/rpi.rs`, possibly `src/core/flow.rs` (one-line addition only if `set_start` is absent)
- **Rollback:** Remove the `set_start` call in `with_research`; remove `set_start` from `Flow` if added.

---

### Phase 3 — Medium (Correctness Gaps and Missing Signals)

#### [M1] `TypedFlow::run` silently swallows `max_steps` breach — no store signal (`src/core/typed_flow.rs`)

- **Problem:** `TypedFlow::run` only emits a `warn!` log on breach. `Flow::run` writes `"error"` into the store so callers can detect truncation programmatically. `TypedFlow` has no equivalent.
- **Fix:** After the `warn!` at line 115 in `TypedFlow::run`, write a sentinel into `TypedStore` (e.g., a boolean field `_flow_truncated: bool` on the generic `T` is impractical — use `store.set_extra("_flow_truncated", true)` if TypedStore has an escape hatch, or document that `run_safe` is the correct API for detecting truncation). Inspect `TypedStore` for an appropriate set method before implementing.
- **Files:** `src/core/typed_flow.rs`
- **Rollback:** Remove the sentinel write.

#### [M3] Implicit `"default"` action coupling between `Workflow` and `Flow` is undocumented (`src/patterns/workflow.rs`, `src/core/flow.rs`)

- **Problem:** `Workflow::connect` registers `"default"` edges. `Flow::run` falls back to `"default"` when `"action"` key is absent. A node forgetting to set `"action"` silently advances instead of halting.
- **Fix:** Documentation-only. Add a `# Warning` section to `Workflow::connect` doc-comment explaining the fallback. Add a note to `Flow::add_edge` doc-comment explaining the `"default"` fallback behavior.
- **Files:** `src/patterns/workflow.rs`, `src/core/flow.rs`
- **Rollback:** Remove the added doc comments.

#### [M4] `TypedFlow`, `TypedStore`, `RpiWorkflow`, `BatchFlow` missing from `prelude` and flat exports (`src/lib.rs`)

- **Problem:** These four public types are re-exported at `src/core/mod.rs` and `src/patterns/mod.rs` but absent from the `prelude` module and top-level flat exports in `src/lib.rs`.
- **Fix:** Add to both the `prelude` module and top-level flat exports in `src/lib.rs`:
  ```
  pub use crate::core::typed_flow::{TypedFlow, TypedNode, SimpleTypedNode, create_typed_node};
  pub use crate::core::typed_store::TypedStore;
  pub use crate::patterns::batchflow::BatchFlow;
  pub use crate::patterns::rpi::RpiWorkflow;
  ```
  Note: `RpiWorkflow` is behind the `skills` feature gate — wrap its export in `#[cfg(feature = "skills")]`.
- **Files:** `src/lib.rs`
- **Rollback:** Remove the four added `pub use` lines from both sections.

#### [M5] `Store::set_f64` silently discards NaN/infinite — return type prevents detection (`src/core/store.rs`)

- **Problem:** `set_f64` returns `()` and silently drops writes for NaN/infinite. The existing doc-comment mentions it but a return-type change would break the API.
- **Fix:** Documentation-only (no API break without explicit approval). Expand the doc-comment to explicitly recommend callers validate `value.is_finite()` before calling and to explain that `JSON` does not support NaN/infinite.
- **Files:** `src/core/store.rs`
- **Rollback:** N/A (doc-only).

---

### Phase 4 — Low (Polish, Ergonomics, Documentation)

#### [L1] No `From<anyhow::Error>` despite `anyhow` being a direct dep (`src/core/error.rs`)

- **Problem:** No `From<anyhow::Error> for AgentFlowError` exists. Users mapping `anyhow` errors need boilerplate.
- **Fix:** Add `impl From<anyhow::Error> for AgentFlowError` using `AgentFlowError::Custom(e.to_string())` (or nearest appropriate variant — confirm variant name in `error.rs` before implementing). No new dependency required.
- **Files:** `src/core/error.rs`
- **Rollback:** Remove the `From` impl.

#### [L2] `Flow::with_start` provides no additional value over `add_node` (`src/core/flow.rs`)

- **Problem:** `with_start` constructs a new `Flow` and calls `add_node` — it sets no edges and is equivalent to `Flow::new()` + `add_node`. The name implies more than it does.
- **Fix:** Documentation-only. Add a note to `with_start`'s doc-comment clarifying it is a shorthand constructor equivalent to `Flow::new()` + `add_node`, and that it does not set edges.
- **Files:** `src/core/flow.rs`
- **Rollback:** Remove added doc note.

#### [L3] `BatchFlow` not re-exported from `lib.rs`

- **Covered by M4.** No separate action needed.

#### [L4] `dyn_clone` `'static` constraint limits composability with borrowed data (`src/core/node.rs`)

- **Problem:** `DynClone` requires `'static`. Users with closures capturing references will encounter opaque errors.
- **Fix:** Documentation-only. Add a `# Note` to the `Node` trait doc-comment warning that all implementations must be `'static` due to `DynClone`, and that closures capturing non-`'static` references will not compile.
- **Files:** `src/core/node.rs`
- **Rollback:** Remove added doc note.

#### [L5] MSRV 1.75 rationale undocumented (`Cargo.toml`)

- **Problem:** `rust-version = "1.75"` has no comment explaining the minimum.
- **Fix:** Add inline comment above `rust-version` in `Cargo.toml`: `# 1.75: async fn in traits (RFC 3185) stabilised`.
- **Files:** `Cargo.toml`
- **Rollback:** Remove the comment.

---

### Execution Order Summary

| Priority | ID | File(s) | Type | Effort |
|---|---|---|---|---|
| 1 | C1 | `parallel.rs` | Bug fix | Small |
| 2 | C2 | `batchflow.rs` | Bug fix | 1 line |
| 3 | H1 | `multi_agent.rs` | Breaking API fix | Small |
| 4 | H4 | `store.rs` | Breaking API fix | Small |
| 5 | H5 | `rpi.rs`, `flow.rs` | Bug fix | Small |
| 6 | H2 | `agent.rs` | Additive API | Small |
| 7 | M1 | `typed_flow.rs` | Signal addition | Small |
| 8 | M3 | `workflow.rs`, `flow.rs` | Doc only | Trivial |
| 9 | M4 | `lib.rs` | Export addition | Trivial |
| 10 | M5 | `store.rs` | Doc only | Trivial |
| 11 | L1 | `error.rs` | Additive impl | Trivial |
| 12 | L2 | `flow.rs` | Doc only | Trivial |
| 13 | L3 | *(covered by M4)* | — | — |
| 14 | L4 | `node.rs` | Doc only | Trivial |
| 15 | L5 | `Cargo.toml` | Doc only | Trivial |

**Breaking changes requiring changelog entry before merge:** H1 (`MergeStrategy::Custom` wraps `fn` → `Arc<dyn Fn>`), H4 (`Store::require_*` return type `String` → `AgentFlowError`).
**No new dependencies required for any item.**

---

## [C1] Fix `clone_store_snapshot` — deep copy instead of Arc clone

**Timestamp:** 2026-03-14T16:34:00Z
**Files modified:** `src/core/parallel.rs`
**Reason:** `clone_store_snapshot` was returning `store.clone()` which only clones the `Arc`, leaving all branches sharing the same `RwLock`. Branch writes were polluting each other, breaking the documented isolation guarantee.

**Previous behavior:** All `ParallelFlow` branches shared the same underlying `RwLock<HashMap>`. Writes in one branch were visible to others; merge results were non-deterministic.

**New behavior:** `clone_store_snapshot` is now `async`. It acquires a read lock, clones the `HashMap`, and wraps it in a fresh `Arc<RwLock<...>>`. Each branch gets a fully independent store. The call site in `run` clones the `Arc` (cheap) to move into the `async move` block, then `await`s the snapshot before the branch executes.

**Rollback:** Revert `clone_store_snapshot` to `fn clone_store_snapshot(store: &SharedStore) -> SharedStore { store.clone() }` and revert the `map` closure to use the old synchronous call without `await`.

---

## [C2] Fix `BatchFlow::run` — `or_insert` → `insert`

**Timestamp:** 2026-03-14T16:35:00Z
**Files modified:** `src/patterns/batchflow.rs`
**Reason:** `or_insert` silently skips writing a param key when it already exists in the store from a previous batch item. Each batch item must unconditionally write its own params so the workflow sees the correct values for that iteration.

**Previous behavior:** `store.entry(k.clone()).or_insert(v.clone())` — first batch item's param values were permanently sticky; all subsequent batch items were silently using stale values.

**New behavior:** `store.insert(k.clone(), v.clone())` — each batch item overwrites the store with its own params before `wf.call()` is invoked.

**Rollback:** Change `store.insert(k.clone(), v.clone())` back to `store.entry(k.clone()).or_insert(v.clone())`.

---

## [H1] `MergeStrategy::Custom` — fn pointer → `Arc<dyn Fn>`

**Timestamp:** 2026-03-14T16:37:00Z
**Files modified:** `src/patterns/multi_agent.rs`
**Reason:** `Custom(fn(Vec<SharedStore>) -> SharedStore)` only accepts bare function pointers. Closures that capture environment produce opaque compiler errors.

**Changes:**
- Added `use std::sync::Arc;`
- `Custom` variant: `fn(...)` → `Arc<dyn Fn(Vec<SharedStore>) -> SharedStore + Send + Sync>`
- `run_custom` param type updated to match
- Match arm: `*merge_fn` (copy) → `merge_fn.clone()` (clone the `Arc`)
- Doc-comments updated; table entry and `Custom` doc updated with `Arc::new(...)` usage example

**Breaking change:** Yes — `MergeStrategy::Custom(my_fn)` call sites must wrap with `Arc::new(my_fn)`.

**Rollback:** Revert the four changes above; remove `use std::sync::Arc` if it was not already present.

---

## [H2] Add `Agent::run_result` — self.node variant of `decide_result`

**Timestamp:** 2026-03-14T16:40:00Z
**Files modified:** `src/patterns/agent.rs`
**Reason:** `decide_result` requires callers to pass an external `node` ref that duplicates `self.node`. `run_result` uses `self.node` directly, matching the ergonomics of `decide_shared`.

**Changes:**
- Added `run_result(&self, input: SharedStore) -> Result<SharedStore, AgentFlowError>` where `N: NodeResult<SharedStore, SharedStore> + Clone`
- Logic is identical to `decide_result` but calls `self.node.call(...)` instead of the external `node` param
- `decide_result` is retained unchanged for backwards compatibility
- Struct-level doc table and link list updated to include `run_result`

**Breaking change:** No — additive only.
**Rollback:** Remove the `run_result` method block and revert the doc edits.

---

## [H4] `Store::require_*` — `Result<_, String>` → `Result<_, AgentFlowError>`

**Timestamp:** 2026-03-14T16:43:00Z
**Files modified:** `src/core/store.rs`
**Reason:** Uniform error contract — callers can now use `?` to propagate into `AgentFlowError` without a manual `.map_err`.

**Changes:**
- Added `use crate::core::error::AgentFlowError`
- `require` → `Result<Value, AgentFlowError>` (missing key: `NotFound`)
- `require_string` → `Result<String, AgentFlowError>` (missing: `NotFound`, wrong type: `TypeMismatch`)
- `require_i64` → `Result<i64, AgentFlowError>` (same pattern)
- `require_f64` → `Result<f64, AgentFlowError>` (same pattern)
- `require_bool` → `Result<bool, AgentFlowError>` (same pattern)
- Each method now holds the read lock for both the existence check and type check (single lock acquisition, correct error variant)
- Struct-level doc comment updated

**Breaking change:** Yes — callers pattern-matching on `String` must update to match `AgentFlowError::NotFound(_)` / `AgentFlowError::TypeMismatch(_)`.

**Rollback:** Revert return types to `Result<_, String>`, remove the `use` import.

---

## [H5] `RpiWorkflow` start node — explicit pin via `Flow::set_start`

**Timestamp:** 2026-03-14T16:46:00Z
**Files modified:** `src/core/flow.rs`, `src/patterns/rpi.rs`
**Reason:** `RpiWorkflow::with_research` relied on being called first so `add_node` would implicitly set it as start. Calling `with_plan` first silently made `"plan"` the start node.

**Changes:**
- `src/core/flow.rs`: Added `pub fn set_start(&mut self, name: &str)` — overwrites `self.start_node` unconditionally.
- `src/patterns/rpi.rs` (`with_research`): calls `self.flow.set_start("research")` after `add_node`, guaranteeing `"research"` is always start regardless of builder order.
- `src/patterns/rpi.rs` (`run`): updated doc-comment to remove the misleading "first node registered" note.

**Breaking change:** No — additive only (`set_start` is new public API; `RpiWorkflow` behaviour is now stricter/correct).
**Rollback:** Remove `set_start` from `flow.rs`; remove the `self.flow.set_start("research")` call in `with_research`.

---

## [M1] `TypedFlow::run` silent `max_steps` breach — add `limit_exceeded` sentinel

**Timestamp:** 2026-03-14T16:50:00Z
**Files modified:** `src/core/typed_store.rs`, `src/core/typed_flow.rs`
**Reason:** `TypedFlow::run` broke out of the loop silently when `max_steps` was reached. Callers had no way to distinguish a completed flow from a truncated one without switching to `run_safe`.

**Changes:**
- `TypedStore<T>`: added `pub limit_exceeded: bool` field (default `false`); added `pub fn limit_exceeded(&self) -> bool` accessor; updated `Clone` impl to copy the flag; updated struct doc-comment with `# Truncation flag` section.
- `TypedFlow::run`: sets `store.limit_exceeded = true` before `break` when `steps >= limit`.
- `TypedFlow::run` doc-comment updated to describe the flag and point to `run_safe`.
- New test `test_typed_flow_run_sets_limit_exceeded_flag` added (asserts flag is `true` and count is `3` after `max_steps = 3` on an infinite loop).

**Breaking change:** `TypedStore` struct gains a new public field — code constructing `TypedStore` with struct literal syntax will need to add `limit_exceeded: false`. Use of `TypedStore::new(…)` is unaffected.
**Rollback:** Remove `limit_exceeded` field and accessor from `TypedStore`; remove `store.limit_exceeded = true` from `TypedFlow::run`; remove the new test.

---

## [M3] `"default"` action fallback coupling — documentation added

**Timestamp:** 2026-03-14T16:55:00Z
**Files modified:** `src/core/flow.rs`, `src/patterns/workflow.rs`
**Reason:** `Flow::run` silently falls back to `"default"` when `store["action"]` is absent. Neither `Flow::add_edge` nor `Workflow::connect` warned callers, so a node forgetting to set `"action"` would silently advance rather than halt.

**Changes (doc-only, no logic):**
- `src/core/flow.rs` (`add_edge`): expanded doc-comment with `# Warning — silent advance on missing "action"` section explaining the `"default"` fallback and how to prevent silent advances.
- `src/patterns/workflow.rs` (`connect`): expanded doc-comment to explain that `connect` registers a `"default"` edge and therefore inherits the same silent-advance risk; added `# Warning` section with mitigation guidance.

**Breaking change:** None (doc-only).
**Rollback:** Revert the two doc-comment expansions.

---

## [M4] `TypedFlow`, `TypedStore`, `RpiWorkflow`, `BatchFlow` added to `prelude` and flat exports

**Timestamp:** 2026-03-14T17:10:00Z
**Files modified:** `src/lib.rs`
**Reason:** Four public types were reachable only via full module paths (`crate::core::typed_flow::TypedFlow`, etc.). Users doing `use agentflow::prelude::*` or `use agentflow::TypedFlow` would get a compile error.

**Changes:**
- `prelude` block: added `TypedFlow`, `TypedNode`, `SimpleTypedNode`, `TransitionFn`, `create_typed_node` from `core::typed_flow`; `TypedStore` from `core::typed_store`; `BatchFlow` from `patterns::batchflow`; `RpiWorkflow` from `patterns::rpi`.
- Flat namespace: identical additions mirroring prelude.
- Crate-level layout doc-comment updated: `TypedStore`, `TypedFlow` now use short intra-doc links; `BatchFlow` and `RpiWorkflow` listed under `patterns`; `skills` feature description corrected (no longer claims `RpiWorkflow` is feature-gated — it is always compiled).

**Note on `RpiWorkflow` feature gate:** PLAN.md previously noted `RpiWorkflow` requires `#[cfg(feature = "skills")]`. Inspection of `src/patterns/rpi.rs` and `src/patterns/mod.rs` shows no such gate — the struct is unconditionally compiled. The `skills` feature only adds `serde_yaml`. No gate was added.

**Breaking change:** None (additive only).
**Rollback:** Remove the eight new `pub use` lines from both `prelude` and the flat namespace block; revert the layout doc-comment.

---

## [M5] `Store::set_f64` silent NaN/infinite discard — expanded doc-comment

**Timestamp:** 2026-03-14T17:20:00Z
**Files modified:** `src/core/store.rs`
**Reason:** `set_f64` had a one-line comment noting the silent-drop behaviour. Callers had no guidance on how to guard against it or why it happens (JSON has no NaN/∞ representation).

**Change (doc-only, no API break):**
Replaced the one-line `/// Insert a float value. Silently does nothing if value is NaN or infinite.` with a full section that:
- Cites RFC 8259 §6 (JSON number spec has no NaN/∞)
- Names `serde_json::Number::from_f64` → `None` as the underlying mechanism
- Calls out that the **key is neither inserted nor updated** (important: not cleared either)
- Provides a concrete `if v.is_finite()` guard pattern in a `rust,no_run` doctest
- Suggests two alternatives when a sentinel is needed: omit the key and treat `get_f64() == None` as the signal, or store as a `"NaN"` string

**Breaking change:** None.
**Rollback:** Revert the doc-comment to the single-line version.

---

## [L1] `From<anyhow::Error>` for `AgentFlowError` added

**Timestamp:** 2026-03-14T17:30:00Z
**Files modified:** `src/core/error.rs`
**Reason:** `anyhow` was already a direct dependency but no `From` impl existed. Callers mapping `anyhow` errors into `AgentFlowError` required manual `.to_string()` boilerplate.

**Change:**
Added `impl From<anyhow::Error> for AgentFlowError` converting into `AgentFlowError::Custom`. Uses `format!("Error: {}", error)` which preserves the full `anyhow` error chain via its `Display` impl (`"outer: inner: cause"`). Pattern is consistent with existing `From<std::io::Error>` and `From<serde_json::Error>` impls in the same file.

Doc-comment explains chain preservation and includes a live doctest (`cargo test --doc` passes — test id `core::error::AgentFlowError::from (line 80)`).

**Breaking change:** None (additive impl).
**Rollback:** Remove the `impl From<anyhow::Error>` block.

---

## [L2] `Flow::with_start` doc clarification

**Timestamp:** 2026-03-14T17:40:00Z
**Files modified:** `src/core/flow.rs`
**Reason:** The old one-liner "Create a flow and immediately register node as both the first node and the start node" implied more than the method does. Callers were unclear it does NOT add edges and is purely a shorthand constructor.

**Change (doc-only, no API break):**
Replaced one-line comment with full section that:
- States equivalence to `Flow::new()` + `add_node()` with a live doctest
- Explicitly calls out what it does NOT do (no edges, no ResultNode support)
- Points callers toward `Flow::new()` + `add_node()` + `add_edge()` + `set_start()` for complex graphs

**Doctest fix:** Initial doctest used wrong `create_node` closure return type (`String` instead of `SharedStore`). Corrected to `|store: SharedStore| async move { store }`.

**Breaking change:** None.

---

## [L4] `Node` / `NodeResult` `'static` constraint documented

**Timestamp:** 2026-03-14T17:50:00Z
**Files modified:** `src/core/node.rs`
**Reason:** `dyn_clone::clone_trait_object!` silently adds a `'static` bound. Users closures capturing non-`'static` references get opaque compiler errors with no hint about the root cause.

**Change (doc-only, no API break):**
- `Node` trait: added `# \`'static\` requirement` section explaining the `dyn_clone` source of the bound, a `compile_fail` doctest demonstrating the exact error, and three concrete workarounds (clone, Arc, move).
- `NodeResult` trait: added a parallel note cross-referencing `Node`'s section.

**Doctest:** `compile_fail` test passes (confirms the constraint is real and the example fails correctly). Total doc-tests: 27/27.

**Breaking change:** None.

---

## [L5] MSRV 1.75 rationale documented

**Timestamp:** 2026-03-14T18:00:00Z
**Files modified:** `Cargo.toml`, `README.md`
**Reason:** `rust-version = "1.75"` had no explanation. Maintainers and contributors had no way to know *why* that floor exists or what would break if it were lowered.

**Changes (doc-only, no API break):**
- `Cargo.toml`: inline comment above `rust-version` naming both stabilised features (AFIT RFC 3185, RPITIT RFC 3425).
- `README.md`: new `## MSRV` section (before `## License`) with a table linking each feature to its RFC and stabilisation version, plus a note about the `rust-version` enforcement and how to pin an older release.

**Breaking change:** None.

## Phase 5: Production Readiness (v1.0)
**Focus:** Hardening the crate for public release on crates.io with strict quality, documentation, and security guarantees.

### Step 5.1: Strict CI Linting for Documentation
* **Timestamp (UTC):** 2026-03-14T19:46:00Z
* **Objective:** Ensure all public APIs are documented and prevent broken intra-doc links from being merged.
* **Tasks:**
  1. Add a `rustdoc` step to `.github/workflows/ci.yml` using `cargo rustdoc -- -D warnings`.
* **Files modified:** `.github/workflows/ci.yml`

### Step 5.2: Crate-Level Code Quality Attributes
* **Timestamp (UTC):** 2026-03-14T19:46:00Z
* **Objective:** Enforce 100% documentation coverage and prevent the use of panicking macros (`unwrap`, `expect`) in library code.
* **Tasks:**
  1. Add `#![warn(missing_docs)]` (or `#![deny(missing_docs)]`) to `src/lib.rs`.
  2. Add `#![warn(clippy::unwrap_used, clippy::expect_used)]` to `src/lib.rs`.
  3. Replace existing `.unwrap()` calls in the codebase with proper error propagation or safe fallbacks.
* **Files modified:** `src/lib.rs`, `src/core/flow.rs`, `src/core/node.rs`, `src/core/store.rs`, `src/utils/tool.rs`

### Step 5.3: Automated Release Pipeline (CD)
* **Timestamp (UTC):** 2026-03-14T19:46:00Z
* **Objective:** Automate the publishing of the crate to `crates.io` when a new version tag is pushed.
* **Tasks:**
  1. Create `.github/workflows/publish.yml`.
  2. Configure it to trigger on `tags: ['v*.*.*']`.
  3. Include steps to checkout, build, test, and run `cargo publish` using a `CARGO_REGISTRY_TOKEN` secret.
* **Files modified:** `.github/workflows/publish.yml` (new)

### Step 5.4: Supply Chain Security Scanning
* **Timestamp (UTC):** 2026-03-14T19:46:00Z
* **Objective:** Automatically detect vulnerabilities in the dependency tree.
* **Tasks:**
  1. Add a `cargo audit` step to the CI pipeline or configure GitHub Dependabot for Cargo.
* **Files modified:** `.github/workflows/ci.yml` or `.github/dependabot.yml`

### Step 5.5: Eliminate Error Boilerplate with `thiserror` (Optional)
* **Timestamp (UTC):** 2026-03-14T19:46:00Z
* **Objective:** Standardize and simplify the `AgentFlowError` implementation.
* **Tasks:**
  1. Add `thiserror` to `Cargo.toml` dependencies.
  2. Refactor `src/core/error.rs` to derive `thiserror::Error` for `AgentFlowError`.
* **Files modified:** `Cargo.toml`, `src/core/error.rs`

---

- **Timestamp (UTC):** 2026-03-14T19:47:00Z
- **Summary of change:** Appended Phase 5 implementation plan for production readiness.
- **Files modified:** `PLAN.md`
- **Exact reason:** User requested an implementation plan to make the project fit for production use as a Rust crate.
- **Previous behavior:** `PLAN.md` ended at Phase 4.
- **New behavior:** `PLAN.md` now includes Phase 5 detailing CI/CD, documentation, code quality, and security improvements.
- **Rollback instructions:** Delete the Phase 5 section and this log entry from `PLAN.md`.

## [2026-03-14T20:45:00Z] Implement Step 5.2: Crate-Level Code Quality Attributes
- **Summary of change:** Enforced `#![warn(missing_docs)]` across the codebase, added missing docstrings to submodules and fields, and ensured no panicking `unwrap()`/`expect()` calls exist in library code.
- **Files modified:**
  - `src/lib.rs`
  - `src/core/mod.rs`
  - `src/core/typed_flow.rs`
  - `src/patterns/mod.rs`
  - `src/patterns/batchflow.rs`
  - `src/skills/mod.rs`
  - `src/mcp/mod.rs`
  - `src/mcp/server.rs`
- **Exact reason:** Required to fulfill Step 5.2 (Production Readiness) and ensure library consumers have full API documentation and safe error boundaries without panic vectors.
- **Previous behavior:** Several types, fields, and modules lacked documentation, triggering compiler warnings after the `#![warn(missing_docs)]` was added.
- **New behavior:** All types and modules are documented; `cargo clippy --all-targets` passes without documentation or unwrap/expect warnings in the library code.
- **Rollback instructions:** Run `git revert HEAD` to remove the documentation additions.

## [2026-03-14T20:55:00Z] Update design system endpoint in MCP examples
- **Summary of change:** Appended a trailing slash to the root URL for the GoA Design System in `mcp_server.rs` and `mcp_client.rs`.
- **Files modified:**
  - `examples/mcp_server.rs`
  - `examples/mcp_client.rs`
- **Exact reason:** User explicitly requested to use the endpoint `https://design.alberta.ca/` in the MCP client and server examples.
- **Previous behavior:** URL was `https://design.alberta.ca` without the trailing slash.
- **New behavior:** URL is `https://design.alberta.ca/` with the trailing slash.
- **Rollback instructions:** Run `git checkout -- examples/` or edit files to remove the trailing slash.
- **2026-03-14T23:45:00Z**: Refactored `examples/mcp_client.rs` to use `AgentFlow`'s `TypedFlow` and `create_typed_node` for agent orchestration and flow management.
  - **Reason**: The existing `mcp_client.rs` was using a manual `loop { match state.next_action { ... } }` construct instead of leveraging the library's built-in `TypedFlow` orchestration primitives.
  - **Previous Behavior**: Hardcoded `loop` over an enum state block.
  - **New Behavior**: Defines `Crawl`, `Review`, and `Report` nodes via `create_typed_node` and wires them together via `flow.add_transition`, running with `flow.run(store)`.
  - **Rollback Instructions**: `git checkout main -- examples/mcp_client.rs`

### Completed
- Refactored `examples/mcp_client.rs` to replace the hardcoded state machine loop with `agentflow::core::TypedFlow`, `TypedStore`, and `create_typed_node` for structured orchestration.
- Verified that all states and transitions in `examples/mcp_client.rs` map cleanly to flow nodes and the routing logic.

### Completed
- Changed all GPT models in `examples/mcp_client.rs` to use `gpt-4.1-mini`.

- Fixed JSON parse error in `examples/mcp_client.rs` by updating the prompt for Agent 1 to clarify that `status` must be an integer HTTP status code.
