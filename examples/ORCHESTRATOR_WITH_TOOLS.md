# Orchestrator with Tools

## What this example is for

This example demonstrates the `Orchestrator with Tools` pattern in AgentFlow.

**Primary AgentFlow pattern:** `Orchestrator + ToolRegistry`  
**Why you would use it:** mix tool use with orchestration decisions.

## How the example works

1. # Example: orchestrator_with_tools.rs
2. Real-world orchestrator that delegates to a ReAct sub-agent. The sub-agent
3. uses a real shell tool (`uname -a`) and passes the result back to the
4. Orchestrator LLM, which then writes a human-readable system summary.
5. Orchestrator (LLM) receives the main task and delegates to the ReAct flow.
6. ReAct Reasoner (LLM) decides to call the `sysinfo` tool.

## Execution diagram

```mermaid
flowchart TD
    A[User request] --> B[Orchestrator node\nLLM decides task + delegates to ReAct]

    subgraph ReAct["Inner ReAct Flow (max 10 steps)"]
        C[reasoner node\nLLM decides: use_tool or finish]
        C -->|use_tool| D[tool node\nToolRegistry: sysinfo = uname -a]
        D -->|default| C
    end

    B --> C
    C -->|finish / no action| E[Write summary to store]
    E --> F([Final answer printed])
```

**AgentFlow patterns used:** `Flow` · `create_node` · `ToolRegistry` · Nested ReAct sub-flow

- The example source is `examples/orchestrator_with_tools.rs`.
- It uses AgentFlow primitives to move data through a store, flow, or higher-level pattern wrapper.
- The implementation is meant to be adapted by swapping in your own prompts, tool handlers, retrieval logic, or business rules.
- When an LLM provider is used, the example relies on `rig` and environment-provided credentials.

## Build your own with this pattern

Use the same pattern in your own project like this:

```rust
// Register allowed tools
let mut registry = ToolRegistry::new();
registry.register("sysinfo", "uname", vec!["-a".into()], None);

// Inner ReAct flow: reasoner decides whether to call a tool
let mut react = Flow::new().with_max_steps(10);
react.add_node("reasoner", reasoner_node);
react.add_node("tool", registry.create_node("sysinfo").unwrap());
react.add_edge("reasoner", "use_tool", "tool");
react.add_edge("tool", "default", "reasoner");

// Outer orchestrator wraps the ReAct flow as a single node
let orchestrator = create_node(move |store: SharedStore| {
    // run the inner react flow, then compose final answer
    Box::pin(async move { store })
});
```

### Customization ideas

- Use this when you need to mix tool use with orchestration decisions.
- Replace the demo prompts, tools, or handlers with your application logic.
- Persist or forward the final result at your system boundary.

## How to run

```bash
cargo run --example orchestrator-with-tools
```

## Requirements and notes

Requires provider credentials plus any tool-specific environment/configuration.
