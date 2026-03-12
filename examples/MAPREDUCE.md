# Example: mapreduce

*This documentation is automatically generated from the source code.*

# Example: mapreduce.rs

**Purpose:**
Shows how to use the MapReduce pattern to process a batch of documents, summarize each with an LLM, and aggregate the results.


## Implementation Architecture

```mermaid
graph TD
    Docs[(Document Array)] --> Map[Mapper Node<br>Parallel execution]
    Map -->|Spawn Task| M1[Mapped Item 1]
    Map -->|Spawn Task| M2[Mapped Item 2]
    M1 --> Reduce[Reducer Node<br>Aggregate]
    M2 --> Reduce
    Reduce --> Output[(Summary Store)]
    
    classDef mapreduce fill:#e0f7fa,stroke:#006064,stroke-width:2px;
    class Map,Reduce mapreduce;
```

**How it works:**
- The mapper agent summarizes each document using an LLM.
- The reducer agent concatenates all summaries into a single string.
- The MapReduce pattern handles the orchestration.

**How to adapt:**
- Use this for any batch processing scenario: batch LLM calls, aggregation, analytics.
- Change the mapper/reducer logic to fit your data and goals.

**Example:**
```rust
let map_reduce = MapReduce::new(batch_mapper, reducer);
let result = map_reduce.run(inputs).await;
```