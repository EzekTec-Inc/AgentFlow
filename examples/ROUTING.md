# Example: routing

*This documentation is automatically generated from the source code.*

# Example: routing.rs

Real-world LLM-powered intent routing. A Triage node calls an LLM to classify
a customer message into one of three intents (tech_support, billing, general)
and routes it to the appropriate specialist agent — also LLM-backed.

Domain: customer service inbox routing.

Requires: OPENAI_API_KEY
Run with: cargo run --example routing

## Implementation Architecture

```mermaid
graph TD
    Input[(User Intent)] --> Router[Router Node<br>Analyzes intent]
    Router -->|action: search| Search[Search Node]
    Router -->|action: write| Write[Write Node]
    Router -->|action: calculate| Calc[Calculate Node]
    Search --> End[(Result)]
    Write --> End
    Calc --> End
    
    classDef route fill:#e3f2fd,stroke:#1565c0,stroke-width:2px;
    class Router route;
```

