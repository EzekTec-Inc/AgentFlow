# AgentFlow v0.3.0 Implementation Plan

> **STATUS: FULLY IMPLEMENTED**
> All four phases of this architectural overhaul have been implemented, tested, and pushed to the repository. The codebase now natively features pre-flight graph validation, enum-based routing (`TypedFlow<T, E>`), lock-free actor message passing, and telemetry contexts.
>
> *This document is preserved as an architectural roadmap and historical record of the v0.3.0 implementation efforts.*

Based on the architectural review of the v0.2.0 codebase, the following four major improvements are planned for the v0.3.0 / v1.0.0 release. These address current hidden bottlenecks and DX friction points.

## Phase 1: Pre-Flight Graph Validation
**Objective:** Catch invalid transitions, typos, and infinite cycles before any LLM calls are executed, rather than failing at runtime.
* **Tasks:**
  1. Add `petgraph` to the crate dependencies.
  2. Create a `flow.validate()` method that builds a directed `petgraph::Graph` from the registered nodes and edges.
  3. **Reachability Check:** Iterate over all stringly-typed edges to ensure the target node exists in the `nodes` map. If an edge points to a missing node, return a `GraphBuildError`.
  4. **Cycle Detection:** Use `petgraph::algo::tarjan_scc` to compute Strongly Connected Components. Identify and flag cycles that could cause infinite loops, providing warnings or requiring explicit cyclic opt-ins.

## Phase 2: Typestate & Enum-Based Routing (`TypedFlow`)
**Objective:** Replace stringly-typed transitions in `TypedFlow` with strongly-typed Enums to guarantee that all possible paths are handled by the compiler.
* **Tasks:**
  1. Define a generic `Transition<E>` enum for node outputs (e.g., `Next`, `Retry`, `Halt`).
  2. Refactor `TypedNode` signatures from `async fn call(store) -> store` to `async fn call(store) -> (store, ActionEnum)`.
  3. Update `TypedFlow::add_edge` to map specific Enum variants to target nodes, eliminating the silent "default" fallback trap.

## Phase 3: Lock-Free Actor-Based Message Passing
**Objective:** Eliminate `Arc<RwLock<T>>` locking contention by transitioning to an actor model where state is passed as owned messages.
* **Tasks:**
  1. Introduce a `NodeWorker` pattern that runs on its own Tokio task.
  2. Use `tokio::sync::mpsc` channels to pass an owned `State` struct from the orchestrator dispatcher to the active node.
  3. Nodes consume the state, perform their async work, and send the modified owned state back to the orchestrator along with the next routing action.
  4. This natively unlocks lock-free concurrent execution for parallel branches.

## Phase 4: First-Class Telemetry & Tracing
**Objective:** Provide built-in observability for LLM token usage, latencies, and generation durations.
* **Tasks:**
  1. Introduce a `FlowContext` struct containing `token_usage`, `start_time`, and active tracing spans.
  2. Update the `Node::call` trait signature to accept `(store, context)`.
  3. Nodes append metrics to the context, allowing the orchestrator to yield a comprehensive telemetry report alongside the final state when `flow.run()` completes.
