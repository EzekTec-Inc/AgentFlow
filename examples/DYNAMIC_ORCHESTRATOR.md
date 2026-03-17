# Example: dynamic_orchestrator

*This documentation is generated from the source code.*

# Example: dynamic_orchestrator.rs

A dynamic orchestrator that reads agent configuration from `examples/agents.toml`
at runtime. If the file does not exist it is created with defaults before proceeding.

How it works:
1. Boot       — load (or create) `examples/agents.toml`, build an AgentRegistry.
2. Planner    — LLM receives the goal + available agent names, returns a JSON array
   of { name, prompt } objects selecting which agents to run and in what order.
3. Dispatcher — pops one AgentSpec per cycle, looks it up in the registry, runs it,
   appends the result; loops until the plan is empty.
4. Aggregator — LLM synthesises every agent result into a final report.

Requires: OPENAI_API_KEY
Run with: cargo run --example dynamic-orchestrator
