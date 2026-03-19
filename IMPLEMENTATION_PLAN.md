# Implementation Plan: Advanced AgentFlow Examples

This plan outlines the creation of three high-impact examples to showcase the full power of the AgentFlow framework. All examples will be implemented strictly adhering to the `strict-project-execution` skill constraints.

---

## Example 1: `security_auditor.rs` (Multi-Agent + MCP + HITL)
**Showcases:** `ParallelFlow`, `SharedStore` merging, Shell Tools, and Human-in-the-Loop.

### Step 1.1: Define the Workflow Graph
1. Create `examples/security_auditor.rs`.
2. Define a `SharedStore` to hold the target repository path (e.g., the current project).
3. **Crawler Node:** Create a node that executes `cargo clippy` and `cargo audit` on the target path using the `ToolRegistry` and stores the raw stdout/stderr.
4. **Analysis Fan-Out (`ParallelFlow`):**
   - **Agent A:** Parses the `clippy` output for logic bugs.
   - **Agent B:** Parses the `audit` output for CVEs.
   - **Agent C:** Uses `grep` (via a registered tool) to scan for hardcoded secrets.
5. **Synthesis Node:** Create a merge strategy that takes the outputs of A, B, and C and formats a Markdown report.
6. **HITL Node:** Implement a `HumanInTheLoop` hook that pauses execution, prints the draft report, and asks the user to "Approve" or "Reject".
7. **Finalization Node:** If approved, write the report to disk. If rejected, halt.

### Step 1.2: Register the Example
1. Add `[[example]]` entry for `security_auditor` in `Cargo.toml`.
2. Ensure the required features (`mcp`, `skills`, `repl`) are enabled.

---

## Example 2: `continuous_rag.rs` (BatchFlow + Rag + SkillInjector)
**Showcases:** Data parallelism (`ParallelBatch`), vector database integration, and dynamic capabilities.

### Step 2.1: Define the Ingestion Pipeline
1. Create `examples/continuous_rag.rs`.
2. Define a list of mock document strings (e.g., Markdown content about the framework).
3. **Batch Processing (`ParallelBatch`):** Create a flow that iterates over the documents concurrently. For each document, an LLM node generates a concise summary.
4. **Vector Ingestion (`Rag` Pattern):** Create a node that takes the summaries, generates embeddings, and pushes them to an in-memory or ephemeral Qdrant collection.

### Step 2.2: Define the Query Pipeline
1. **Skill Injection:** Use a `SkillInjector` to dynamically equip a "Query Agent" with a `query_qdrant` tool.
2. **Execution:** Ask the Query Agent a question related to the ingested documents. The agent should use its new skill to retrieve the relevant summaries and formulate an answer.

### Step 2.3: Register the Example
1. Add `[[example]]` entry for `continuous_rag` in `Cargo.toml`.
2. Ensure the `rag` feature is enabled.

---

## Example 3: `self_correcting_coder.rs` (Agent Retries + StateDiff)
**Showcases:** Typed error boundaries (`AgentFlowError`), state-diffing, and built-in agent retry logic.

### Step 3.1: Define the Generation and Evaluation Loop
1. Create `examples/self_correcting_coder.rs`.
2. **Prompt Setup:** Define a prompt asking the LLM to write a specific Rust function (e.g., a function that calculates the nth Fibonacci number) using `rig-core`.
3. **Generator Node:** Create an `Agent` node that calls the LLM and extracts the code block from the response. Use `create_diff_node` to safely update the `SharedStore`.
4. **Evaluation Node (`ResultNode`):** Create a node that writes the code to a temporary file in the OS `temp_dir` and attempts to compile it using `rustc`.
   - If compilation succeeds, return `Ok(())`.
   - If compilation fails, return `Err(AgentFlowError::NodeFailure(compiler_stderr))`.

### Step 3.2: Configure the Agent Retries
1. Configure the `Agent` with `max_retries` (e.g., 3).
2. Ensure the framework's retry loop catches the `NodeFailure`, appends the stderr to the LLM's prompt context, and executes the Generator Node again without halting the entire process.

### Step 3.3: Register the Example
1. Add `[[example]]` entry for `self_correcting_coder` in `Cargo.toml`.