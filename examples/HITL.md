# Human-in-the-Loop (HITL) Tutorial

## What this example is for

This example demonstrates the native **Human-in-the-Loop (HITL)** node pattern in AgentFlow. 

**Primary AgentFlow pattern:** `Suspended Execution`  
**Why you would use it:** To pause an automated LLM flow indefinitely until a specific piece of data (like human approval, a webhook payload, or manual text input) is provided. This is crucial for safely orchestrating agents that perform destructive actions (like deploying code, sending emails, or making payments).

## How it works

The core of this pattern relies on `AgentFlowError::Suspended`. 
1. We use the built-in `create_hitl_node` function to create an approval gate. 
2. This node checks the `SharedStore` for a specific key (e.g., `"human_approval"`).
3. If the key is missing, the node immediately halts execution and returns an `Err(Suspended)`.
4. The host application catches this error, waits for the user (via CLI input, API, etc.), injects the missing key into the store, and calls `.run_safe()` again.
5. The flow resumes exactly where it left off.

### Step-by-Step Code Walkthrough

First, we set up our three nodes. Step 1 does some initial work. Step 2 is the HITL gate. Step 3 is the final step that requires approval.

```rust
// 1. Initial Processing Node
flow.add_node("step1", create_node(|store: SharedStore| { /* ... */ }));

// 2. HITL Gate Node
// Arguments: 
// - The key to look for in the store ("human_approval")
// - The routing action to take if the key exists ("final_step")
// - The reason string returned when suspended
flow.add_result_node(
    "approval_gate",
    create_hitl_node("human_approval", "final_step", "Awaiting human approval"),
);

// 3. Final Processing Node
flow.add_node("final_step", create_node(|store: SharedStore| { /* ... */ }));
```

Next, we run the flow. The first time we run it, it halts at the `approval_gate` because `"human_approval"` is not in the store. We catch the `Suspended` error using `.run_safe()`.

```rust
let store = Arc::new(RwLock::new(HashMap::new()));

// Run 1: Should suspend because "human_approval" is missing.
let result1 = flow.run_safe(store.clone()).await;

match result1 {
    Err(AgentFlowError::Suspended(reason)) => {
        println!("Flow suspended correctly: {}", reason);
    }
    _ => println!("Unexpected result!"),
}
```

Finally, we simulate the human interaction. We write the missing key (`"human_approval"`) into the `SharedStore` and call `.run_safe()` a second time. The orchestrator automatically resumes execution at the `approval_gate`, sees the key is now present, and routes to `"final_step"`.

```rust
// Simulate Human Interaction
let mut guard = store.write().await;
guard.insert("human_approval".to_string(), json!(true));
drop(guard);

// Run 2: Resume the flow
let final_store = flow.run_safe(store).await.unwrap();
```

## How to run

Run the example using cargo. It doesn't require an LLM API key since it just demonstrates the state machine mechanics:

```bash
cargo run --example hitl
```