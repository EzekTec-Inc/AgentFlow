# Contributing to AgentFlow

Thank you for your interest in contributing. This guide covers everything you need to get started quickly.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Getting the Code](#getting-the-code)
3. [Project Layout](#project-layout)
4. [Running Tests](#running-tests)
5. [Running Examples](#running-examples)
6. [Coding Conventions](#coding-conventions)
7. [Adding a New Pattern or Feature](#adding-a-new-pattern-or-feature)
8. [Adding or Updating an Example](#adding-or-updating-an-example)
9. [Pull Request Process](#pull-request-process)
10. [Commit Message Format](#commit-message-format)

---

## Prerequisites

| Tool | Minimum version | Install |
|------|----------------|---------|
| Rust | 1.75 | `rustup update stable` |
| `cargo fmt` | (bundled) | — |
| `cargo clippy` | (bundled) | — |

No other tooling is required for the core crate. Feature-gated modules (`rag`, `mcp`, `skills`, `repl`) may require additional system libraries — see each feature's section in `Cargo.toml`.

---

## Getting the Code

```bash
git clone https://github.com/<your-fork>/AgentFlow.git
cd AgentFlow
cargo check --all-features
```

---

## Project Layout

```
src/core/         SharedStore, Flow, TypedFlow, ParallelFlow, Batch,
                  StateDiff, create_diff_node, AgentFlowError
src/patterns/     Agent, Workflow, MultiAgent, MapReduce, Rag,
                  StructuredOutput, RpiWorkflow
src/utils/        create_tool_node, ToolRegistry,
                  create_corrective_retry_node
src/skills/       (feature: skills) YAML skill parser
src/mcp/          (feature: mcp) MCP stdio server
examples/         One runnable example per pattern
tests/            Integration tests
```

See `ARCHITECTURE.md` for the full design overview.

---

## Running Tests

```bash
# Default features
cargo test

# All features
cargo test --all-features

# A single test
cargo test <test_name>
```

All tests must pass before a PR is merged. CI runs `cargo test --all-features` on every push and pull request.

---

## Running Examples

```bash
export OPENAI_API_KEY=sk-...

cargo run --example agent
cargo run --example workflow
cargo run --example multi-agent
cargo run --example rag
cargo run --example mapreduce
cargo run --example orchestrator-multi-agent
cargo run --example orchestrator-with-tools
cargo run --example structured-output
cargo run --example error-handling
cargo run --example react
cargo run --example reflection
cargo run --example plan-and-execute
cargo run --example routing
cargo run --example repl
cargo run --example typed-flow
cargo run --example dynamic-orchestrator
cargo run --example rpi
cargo run --example rust-agentic-skills --features skills
cargo run --example document-processing  --features skills
```

---

## Coding Conventions

- **Format:** `cargo fmt` before every commit.
- **Lint:** `cargo clippy -- -D warnings` must produce zero warnings.
- **Async:** Never hold a `SharedStore` lock across `.await`. Use `create_diff_node` for async node logic that reads state — it receives a snapshot with no lock held.
- **Error handling:** Use `AgentFlowError` variants; avoid `unwrap()` in library code.
- **Docs:** Every public item must have a `///` doc comment. Include a short `# Example` block for non-trivial types and functions.
- **Tests:** Unit tests go in `#[cfg(test)]` modules inside the source file. Integration tests go in `tests/`.

---

## Adding a New Pattern or Feature

1. Create `src/patterns/<name>.rs` (or `src/core/<name>.rs` for primitives).
2. Add `pub mod <name>;` in the parent `mod.rs`.
3. Re-export from `src/lib.rs` and `src/prelude.rs` if user-facing.
4. Add at least one unit test.
5. Add a corresponding example in `examples/<name>.rs` and register it in `Cargo.toml`.
6. Create `examples/<NAME>.md` documenting the example (see below).
7. Update `ARCHITECTURE.md` with any new types or flow changes.
8. Append a changelog entry to `PLAN.md`.

### New primitives checklist (`core/`)

- [ ] `StateDiff` / `create_diff_node` pattern considered for lock safety
- [ ] `AgentFlowError` variant added if a new failure mode is introduced
- [ ] `ParallelFlow` / `TypedFlow` compatibility verified
- [ ] Exported from `prelude`

### New `utils/tool` entry checklist

- [ ] Added to `ToolRegistry` allowlist in example
- [ ] Documented timeout behaviour
- [ ] `create_corrective_retry_node` used if LLM self-correction is needed

---

## Adding or Updating an Example

1. Add `examples/<name>.rs` with a top-level `/*!` doc comment covering:
   - **Purpose** — one sentence.
   - **How it works** — bullet list.
   - **How to adapt** — bullet list.
   - `Requires:` and `Run with:` lines.

2. Register the example in `Cargo.toml`:
   ```toml
   [[example]]
   name = "my-example"
   path = "examples/my_example.rs"
   ```

3. Create `examples/MY_EXAMPLE.md` with:
   - Title and auto-gen notice.
   - The full doc comment from the source.
   - A `## Implementation Architecture` section with a Mermaid diagram.

4. Run the example end-to-end and confirm it works.

---

## Pull Request Process

1. Fork the repo and create a feature branch: `git checkout -b feat/my-feature`.
2. Make your changes following the conventions above.
3. Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test --all-features`.
4. Push your branch and open a PR against `main`.
5. Fill in the PR template: motivation, changes made, how to test.
6. A maintainer will review within a few business days.

---

## Commit Message Format

```
<type>(<scope>): <short description>

[optional body]

[optional footer]
```

**Types:** `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`

**Examples:**
```
feat(core): add ParallelFlow fan-out / fan-in
fix(utils): prevent LLM-injected tool names via ToolRegistry
docs(examples): update PLAN_AND_EXECUTE.md with corrective-retry note
chore: bump rig-core to 0.16.0
```
