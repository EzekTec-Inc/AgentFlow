# Example: react

*This documentation is automatically generated from the source code.*

# Example: react.rs

Real-world ReAct (Reason + Act) agent. The LLM decides each turn whether
to call a tool or emit a final answer. Tool execution is a real shell command
(curl-based web fetch is simulated here — swap for any HTTP call).

Requires: OPENAI_API_KEY
Run with: cargo run --example react

## Implementation Architecture

```mermaid
graph TD
    Goal[(Input Goal)] --> Reason[Reason Node<br>Think & Decide]
    Reason --> Act[Act Node<br>Tool Call]
    Act --> Observe[Observe Node<br>Parse Tool Output]
    Observe -->|Not Finished| Reason
    Observe -->|Finished| Output[(Final Response)]
    
    classDef react fill:#fffde7,stroke:#fbc02d,stroke-width:2px;
    class Reason,Act,Observe react;
```

