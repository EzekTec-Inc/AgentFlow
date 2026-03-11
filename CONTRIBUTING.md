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

No other tooling is required for the core crate. Feature-gated modules (`rag`, `mcp`) may require additional system libraries — see each feature's section in `Cargo.toml`.

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
src/core/       — SharedStore, Flow, TypedFlow, Batch, Store, AgentFlowError
src/patterns/   — Agent, Workflow, MultiAgent, MapReduce, Rag, StructuredOutput
src/utils/      — Shell tool nodes
src/skills/     — (feature: skills) YAML skill parser
src/mcp/        — (feature: mcp) MCP stdio server
examples/       — One runnable example per pattern
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
cargo run --example agent
cargo run --example workflow
cargo run --example multi_agent
# etc.
```

All examples are listed in `Cargo.toml` under `[[example]]` sections. Each example is self-contained and requires no external setup unless it calls an LLM API (those read keys from environment variables documented at the top of the file).

---

## Coding Conventions

### Formatting & Linting

```bash
cargo fmt --all          # format
cargo clippy --all-targets --all-features -- -D warnings   # lint
```

Both are enforced by CI. PRs that fail either check will not be merged.

### Style Rules

- **`pub` only what users need.** Internal helpers use `pub(crate)`.
- **`Result` everywhere failures are possible.** Never silently swallow errors; surface them as `AgentFlowError`.
- **Builder pattern for structs with optional config.** Follow the existing `Agent::with_retry`, `Flow::with_max_steps` style.
- **Doc-comments on every public item.** Use `///` with a one-line summary, then an optional longer explanation, then a `# Example` block where helpful.
- **No `unwrap()` or `expect()` in library code.** Use `?` or explicit error mapping.
- **No blocking calls inside async functions.** Use `tokio::task::spawn_blocking` if you must call a blocking API.
- **Never hold a lock guard across an `.await` point.** See `ARCHITECTURE.md` — Concurrency & Safety Rules.

### Error Handling

- Use `AgentFlowError` variants consistently:
  - `NotFound` — missing key or resource.
  - `Timeout` — transient; will be retried by `Agent`.
  - `NodeFailure` — fatal; retries are skipped.
  - `TypeMismatch` — wrong value type in the store.
  - `Custom` — catch-all for one-off cases.
- Add new variants to `AgentFlowError` if an existing one doesn't fit — don't use `Custom` for recurring error categories.

### The `"action"` Key

`"action"` is **reserved** for `Flow` routing. Never use it to store application data. See `ARCHITECTURE.md` — Routing Model.

---

## Adding a New Pattern or Feature

1. Create `src/patterns/<name>.rs` (or `src/core/<name>.rs` for primitives).
2. Add `pub mod <name>;` in `src/patterns/mod.rs` (or `src/core/mod.rs`).
3. Re-export from `src/lib.rs` (both `prelude` and flat namespace) if it is part of the public API.
4. Write doc-comments on every public item with at least one `# Example`.
5. Add a corresponding example in `examples/<name>.rs` and register it in `Cargo.toml`:
   ```toml
   [[example]]
   name = "<name>"
   path = "examples/<name>.rs"
   ```
6. Run `cargo test --all-features` and `cargo clippy --all-features -- -D warnings`.

### Feature-Gated Code

Wrap optional code in `#[cfg(feature = "<flag>")]`. Add the feature to `Cargo.toml` `[features]` with a clear description. Document it in `ARCHITECTURE.md` — Feature Flags and in `src/lib.rs`.

---

## Adding or Updating an Example

- Each example must be **runnable with zero manual setup** (no hard-coded API keys, no external services unless clearly documented at the top).
- If the example calls an LLM, document the required environment variables at the top of the file:
  ```rust
  // Required environment variables:
  //   OPENAI_API_KEY — your OpenAI API key
  ```
- Keep examples focused on one pattern or concept.
- Do **not** duplicate `[[example]]` entries in `Cargo.toml` — one entry per file.

---

## Pull Request Process

1. Fork the repo and create a descriptive branch:
   ```
   git checkout -b feat/middleware-hooks
   git checkout -b fix/flow-cycle-detection
   ```
2. Make your changes following the conventions above.
3. Run the full check suite locally:
   ```bash
   cargo fmt --all
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-features
   ```
4. Open a PR against `main`. Fill in the PR template:
   - **What** — what does this change?
   - **Why** — what problem does it solve?
   - **How** — brief description of the approach.
   - **Breaking changes** — list any API removals or renames.
5. At least one maintainer review is required before merge.
6. Squash-merge is preferred to keep `main` history linear.

---

## Commit Message Format

```
<type>(<scope>): <short summary>

[optional body]

[optional footer: Breaking change / Closes #issue]
```

| Type | Use for |
|------|---------|
| `feat` | New feature or pattern |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `refactor` | Code restructuring without behaviour change |
| `test` | Adding or fixing tests |
| `chore` | Build, CI, dependency updates |

**Examples:**
```
feat(patterns): add middleware hook system to Agent
fix(flow): prevent cycle when action key matches start node
docs(arch): update routing model section in ARCHITECTURE.md
```
