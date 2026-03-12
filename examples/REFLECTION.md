# Example: reflection

*This documentation is automatically generated from the source code.*

# Example: reflection.rs

Real-world Reflection pattern. A Generator LLM writes a draft; a Critic LLM
reviews it and either approves or sends it back with specific feedback. The
loop continues until the Critic approves or max_steps is reached.

Domain: technical blog post paragraph about Rust's ownership model.

Requires: OPENAI_API_KEY
Run with: cargo run --example reflection

## Implementation Architecture

```mermaid
graph TD
    Input[(Initial Prompt)] --> Draft[Draft Node<br>Initial Output]
    Draft --> Critique[Critique Node<br>LLM evaluates draft]
    Critique -->|Needs Improvement| Revise[Revise Node<br>Fix issues]
    Revise --> Critique
    Critique -->|Good Enough| End[(Final Polish)]
    
    classDef reflect fill:#fbe9e7,stroke:#d84315,stroke-width:2px;
    class Draft,Critique,Revise reflect;
```

