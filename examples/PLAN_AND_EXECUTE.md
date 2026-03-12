# Example: plan_and_execute

*This documentation is automatically generated from the source code.*

# Example: plan_and_execute.rs

Real-world Plan-and-Execute agent. A Planner LLM breaks a high-level goal into
numbered steps. An Executor LLM processes each step in turn, popping it off the
plan and producing a result. When the plan is empty the flow terminates.

Domain: writing a short technical report on a user-supplied topic.

Requires: OPENAI_API_KEY
Run with: cargo run --example plan-and-execute

## Implementation Architecture

```mermaid
graph TD
    Goal[(Goal)] --> Planner[Planner Node<br>Create steps]
    Planner --> Executor[Executor Node<br>Execute next step]
    Executor --> Eval[Evaluator Node<br>Check if done]
    Eval -->|Not Done| Executor
    Eval -->|Done| End[(Output Store)]
    
    classDef plan fill:#e8eaf6,stroke:#283593,stroke-width:2px;
    class Planner,Executor,Eval plan;
```

