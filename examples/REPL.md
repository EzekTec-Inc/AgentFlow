# Example: repl

*This documentation is automatically generated from the source code.*

# Example: repl.rs

Real-world LLM-powered REPL. The user types a message; an LLM answers;
the conversation history is kept in the store so the LLM has full context.

Type `exit` or `quit` to stop.

Requires: OPENAI_API_KEY
Run with: cargo run --example repl

## Implementation Architecture

```mermaid
graph TD
    User([User Input]) --> Wait[Wait for Input]
    Wait --> Agent[Agent Node<br>LLM Response]
    Agent --> Output([Stdout])
    Output --> Wait
    
    classDef repl fill:#e0e0e0,stroke:#424242,stroke-width:2px;
    class Wait,Agent repl;
```

