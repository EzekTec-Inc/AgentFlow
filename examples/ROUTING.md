# Conditional Routing Tutorial

## What this example is for

This example demonstrates the **Conditional Routing** pattern in AgentFlow. It builds a real-world, LLM-powered triage system that classifies incoming customer service messages and routes them to the appropriate specialist agent.

**Primary AgentFlow pattern:** `Flow routing`  
**Why you would use it:** To dynamically drive graph transitions based on runtime decisions (like an LLM classifying intent) rather than static sequences.

## How it works

The system is built as a state machine (`Flow`) with four nodes:
1. **`triage`**: The starting node. An LLM reads the user's message and classifies it as `tech_support`, `billing`, or `general`.
2. **`tech_support`**: An LLM agent specialized in technical issues.
3. **`billing`**: An LLM agent specialized in billing queries.
4. **`general`**: A fallback LLM agent for general inquiries.

### Step-by-Step Code Walkthrough

First, we create the **Triage Node**. This node's only job is to evaluate the message, determine the intent, and write that intent to the `action` key in the shared state store.

```rust
let triage = create_node(|store: SharedStore| {
    Box::pin(async move {
        // 1. Read the user's message from the store
        let message = {
            let g = store.read().await;
            g.get("message").unwrap().to_string()
        };

        // 2. Call the LLM to classify the intent
        let agent = client.agent("gpt-4o-mini").preamble(TRIAGE_SYSTEM).build();
        let intent = agent.prompt(&message).await.unwrap_or("general".into());

        // 3. Write the intent as the "action" key
        store.write().await.insert("action".to_string(), Value::String(intent));
        store
    })
});
```

Next, we create a **Specialist Node** (e.g., `tech_support`). It reads the original message, generates a specialized response, and saves it to the store.

```rust
let tech_node = create_node(|store: SharedStore| {
    Box::pin(async move {
        let msg = store.read().await.get("message").unwrap().to_string();
        
        let reply = llm_reply(TECH_SYSTEM, &msg).await;
        
        // Write the final response to the store
        store.write().await.insert("response".to_string(), Value::String(reply));
        store
    })
});
```

Finally, we construct the graph by registering the nodes and defining the **conditional edges**:

```rust
let mut flow = Flow::new();

// The first node added becomes the starting node automatically
flow.add_node("triage", triage);
flow.add_node("tech_support", tech_node);
flow.add_node("billing", billing_node);
flow.add_node("general", general_node);

// Routing logic: If triage outputs "action": "tech_support", go to the tech_support node.
flow.add_edge("triage", "tech_support", "tech_support");
flow.add_edge("triage", "billing", "billing");
flow.add_edge("triage", "general", "general");
```

## How to run

Ensure you have your `OPENAI_API_KEY` set in your environment or `.env` file, then run:

```bash
cargo run --example routing
```