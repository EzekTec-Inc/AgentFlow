# MapReduce Tutorial

## What this example is for

This example demonstrates the **MapReduce** orchestration pattern in AgentFlow. 

**Primary AgentFlow pattern:** `MapReduce`  
**Why you would use it:** When you have a batch of inputs (like a list of documents, a collection of codebase files, or an array of search results) that need to be processed independently by an LLM in parallel (the **Map** phase), and then aggregated or summarized into a single final output (the **Reduce** phase).

## How it works

The `MapReduce` struct orchestrates two distinct node types:
1. **Mapper Node**: A standard `Node` that processes a single `SharedStore` and returns a modified `SharedStore`. In this example, it extracts a document, asks an LLM to summarize it, and writes the summary back. This is wrapped in a `Batch` executor to fan it out concurrently.
2. **Reducer Node**: A special `BatchNode` that receives a `Vec<SharedStore>` (the outputs of all the mappers) and returns a single `SharedStore`. In this example, it concatenates all the individual summaries into one giant string.

### Step-by-Step Code Walkthrough

First, we define the **Mapper** node. This is a standard asynchronous node that extracts a document, calls the OpenAI API to summarize it, and saves the summary.

```rust
let mapper = create_node(|store: SharedStore| {
    Box::pin(async move {
        // Read the document from the state
        let doc = {
            let guard = store.write().await;
            guard.get("doc").unwrap().to_string()
        };

        // Call the LLM
        let openai_client = providers::openai::Client::from_env();
        let rig_agent = openai_client.agent("gpt-4o-mini").build();
        let prompt = format!("Summarize: {}", doc);
        let summary = rig_agent.prompt(&prompt).await.unwrap();

        // Save the summary
        store.write().await.insert("summary".to_string(), Value::String(summary));
        store
    })
});
```

Next, we define the **Reducer** node. Unlike a normal node, `create_batch_node` accepts a `Vec<SharedStore>` as its input. We iterate through all the stores (which have now been processed by the mappers), extract their summaries, and join them together.

```rust
let reducer = create_batch_node(|stores: Vec<SharedStore>| {
    Box::pin(async move {
        let mut all_summaries = Vec::new();
        
        // Collect summaries from every mapper's output
        for s in &stores {
            let summary = s.write().await.get("summary").unwrap().to_string();
            all_summaries.push(summary);
        }
        
        // Create a new final output store
        let mut result = HashMap::new();
        result.insert("all_summaries".to_string(), Value::String(all_summaries.join("\n")));
        Arc::new(tokio::sync::RwLock::new(result))
    })
});
```

Finally, we compose the `MapReduce` workflow. We wrap the `mapper` in a `Batch` struct (which handles the concurrent execution), and pass both to `MapReduce::new()`.

```rust
let batch_mapper = Batch::new(mapper);
let map_reduce = MapReduce::new(batch_mapper, reducer);

// Pass an array of stores (one for each document) to the MapReduce executor
let result = map_reduce.run(inputs).await;
```

## How to run

Ensure you have your `OPENAI_API_KEY` set in your environment or `.env` file, then run:

```bash
cargo run --example mapreduce
```