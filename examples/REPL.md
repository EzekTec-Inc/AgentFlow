# Interactive REPL

## What this example is for

This example demonstrates the `Interactive REPL` pattern in AgentFlow.

**Primary AgentFlow pattern:** `REPL shell`  
**Why you would use it:** Wrap AgentFlow patterns in an interactive loop.

## How the example works

1. Run with: `cargo run --example repl`.
2. The example initializes a flow composed of three core nodes: `read`, `eval`, and `print`.
3. The `read` node collects user input from stdin and stores it in the shared store.
4. The `eval` node runs your AgentFlow workflow or pattern using the stored input.
5. The `print` node renders and prints results back to the terminal.
6. The loop continues until the user enters a quit command (for example, `:quit`).

## Execution diagram

```mermaid
flowchart TD
    A[User command] --> B[Parse input]
    B --> C[Run AgentFlow pattern]
    C --> D[Display result]
    D --> A
```

## Key implementation details

- The example source is `examples/repl.rs`.
- It uses AgentFlow primitives to move data through a store, flow, or higher-level pattern wrapper.
- The implementation is meant to be adapted by swapping in your own prompts, tool handlers, retrieval logic, or business rules.
- When an LLM provider is used, the example relies on `rig` and environment-provided credentials.

## Build your own with this pattern

Use the same pattern in your own project like this:

```rust
loop {
    let line = read_user_input()?;
    if line == ":quit" { break; }
    let result = flow.run(user_store(line)).await?;
    println!("{}", render(result));
}
```

### Customization ideas

- Use this when you need to wrap AgentFlow patterns in an interactive loop.
- Replace the demo prompts, tools, or handlers with your application logic.
- Persist or forward the final result at your system boundary.

## How to run

```bash
cargo run --example repl
```

## Requirements and notes

Requirements depend on the pattern wired into the REPL session.
