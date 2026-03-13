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
