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
