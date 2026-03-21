# Example: continuous_rag

*This documentation is derived from the source code.*

# Example: continuous_rag.rs

**Purpose:**
Demonstrates data parallelism (`ParallelBatch`), vector database ingestion, and dynamic capabilities using the `Rag` and `SkillInjector` patterns.

**How it works:**
- **Data Ingestion:** Uses `ParallelBatch` to iterate over a list of mock documents concurrently. An agent summarizes each document, simulating a high-throughput ingestion stream.
- **Vector DB:** The summaries are "embedded" and pushed to an in-memory mock vector database (simulating Qdrant or Pinecone).
- **Skill Injection:** A `SkillInjector` dynamically equips a new Query Agent with a `query_qdrant` mock skill, providing it instructions on how to search the database.
- **RAG Pipeline:** The `Rag` pattern is employed. A Retriever node fetches the relevant summaries (context) from the DB, and a Generator node uses the context, the user's query, and its dynamically injected skills to formulate an expert answer.

**How to adapt:**
- Adapt this structure for real-time document processing pipelines, chat-with-your-docs applications, or systems that need to process large batches of text before answering questions.

**Example execution:**
```bash
cargo run --features="rag" --example continuous_rag
```
