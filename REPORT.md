# AgentFlow — Business Overview

## Architecture Overview

AgentFlow is built around three core components that work together to orchestrate AI-powered workflows:

- **Agents** — Specialized async units that do the actual work (LLM calls, data retrieval, tool execution, etc.)
- **Workflow / Flow Engine** — Receives requests from the user and manages how Agents are chained together into pipelines, including conditional branching and routing
- **Shared Store** — A central, thread-safe data structure that all Agents read from and write to; this is how Agents pass information to one another

The user interacts with the Flow Engine (via API or CLI). The Flow Engine orchestrates the Agents. All Agents share a single Store as their communication bus.

---

## How the Workflow Works (Plain English)

Think of it like an **assembly line in a factory**:

- **Each station on the line is a Node (Agent)** — it receives a work order (the Shared Store), does its job, marks what it did, then passes it to the next station.

- **The assembly line itself is the Workflow** — it decides the order of stations and which station to go to next based on what the previous station wrote on the work order.

- **The work order is the Shared Store** — it travels with the product through every station. Each station can read what previous stations did and add their own notes.

- **The routing decision is the `"action"` key** — when a station finishes, it writes a tag on the work order (e.g. `"approved"`, `"rejected"`, `"needs_review"`). The workflow reads that tag to decide which station comes next — so the line can **branch** depending on the outcome.

---

## Example in Plain English

> *Build a Rust function* task enters the workflow:
>
> 1. **Reason station** — reads the task, writes down its analysis
> 2. **Plan station** — reads the analysis, writes a step-by-step plan
> 3. **Implement station** — reads the plan, writes the final code

Each station only does its one job. The work order carries everything forward. If the Reason station decides the task is too vague, it can tag the work order `"needs_clarification"` and the line routes it to a **Clarify station** instead of Plan.

That's the whole idea — **simple steps, shared context, smart routing**.

---

## Summary

| Concept | Business Analogy |
|---|---|
| Node / Agent | A station on an assembly line |
| Workflow / Flow Engine | The assembly line itself |
| Shared Store | The work order that travels with the product |
| `"action"` key | The routing tag that determines the next station |
| Conditional branching | Diverting the line based on the outcome of a station |
