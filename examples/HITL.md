# Example: hitl

*This documentation is generated from the source code.*

# Example: hitl.rs

**Purpose:**
Demonstrates the native Human-in-the-Loop (HITL) pattern, which suspends execution until specific input is provided.

**How it works:**
- Creates a flow with a standard node and a HITL node.
- The HITL node is configured to check for the `human_approval` key.
- The first run suspends because the key is missing.
- After simulating human input by inserting the key into the store, the second run succeeds.

**How to adapt:**
- Use `create_hitl_node` in your flows to pause for external input (e.g., API webhook, user CLI input).
- Catch `AgentFlowError::Suspended` to handle the pause gracefully.

**Example:**
```rust
flow.add_result_node("hitl", create_hitl_node("human_approval", "next_step", "Need approval"));
```
