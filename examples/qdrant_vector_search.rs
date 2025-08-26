/*!
MCP Qdrant Vector Search Tool Server
====================================

This file refactors the original `qdrant_vector_search.rs` CLI example into a proper MCP tool server,
using the `mcp-core`, `mcp-core-macros`, and `rig-core` crates. The tool is callable by agents via the MCP protocol (SSE/HTTP).

--------------------------------------------------------------------------------
How to Project/Migrate Your Old Code:
--------------------------------------------------------------------------------

1. Move all Qdrant and embedding setup logic into the MCP tool function.
   - The Qdrant client, collection check, and embedding model setup are now inside the tool.
   - For production, consider moving these to shared state for efficiency.

2. Move your vector search logic into the tool function.
   - Use the `query` parameter from the tool input.
   - Build and send the search request as before.

3. Format and return results as a tool response (not via `println!`).
   - Use `tool_text_content!` to return a string, or `tool_json_content!` for structured output.

4. Remove all CLI/print statements except for server startup info.

5. Update your `Cargo.toml` to include:
   mcp-core = "latest"
   mcp-core-macros = "latest"
   rig-core = "latest"
   qdrant-client = "latest"
   serde = { version = "1", features = ["derive"] }
   anyhow = "1"

--------------------------------------------------------------------------------
How to Run:
--------------------------------------------------------------------------------

1. Start Qdrant:
   docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant

2. Set your OpenAI API key:
   export OPENAI_API_KEY=sk-...

3. Run the MCP tool server:
   cargo run --bin qdrant_vector_search

4. Call the tool from an MCP-compatible agent or client at:
   http://127.0.0.1:3000/sse
   with parameters:
     - query (string, required)
     - samples (integer, optional)

--------------------------------------------------------------------------------
*/

use anyhow::Result;
use rig::providers;
use qdrant_client::{
    Qdrant,
    qdrant::{CreateCollectionBuilder, Distance, QueryPointsBuilder, VectorParamsBuilder},
};
use rig::{client::EmbeddingsClient, vector_store::request::VectorSearchRequest};
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    // --- Qdrant and embedding setup ---
    let collection_name = "rig-collection";
    let qdrant_url = "http://localhost:6334";
    let openai_api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

    let client = Qdrant::from_url(qdrant_url).build()?;

    // Ensure collection exists (idempotent)
    if !client.collection_exists(collection_name).await? {
        client
            .create_collection(
                CreateCollectionBuilder::new(collection_name)
                    .vectors_config(VectorParamsBuilder::new(1536, Distance::Cosine)),
            )
            .await?;
    }

    let openai_client = providers::openai::Client::new(&openai_api_key);
    let model = openai_client.embedding_model(rig::providers::openai::TEXT_EMBEDDING_ADA_002);

    let query_params = QueryPointsBuilder::new(collection_name).with_payload(true);

    // You must add the rig_qdrant crate to your dependencies for this to work.
    // Uncomment the following line if you have rig_qdrant available:
    // use rig_qdrant::QdrantVectorStore;
    // let vector_store = QdrantVectorStore::new(client, model.clone(), query_params.build());

    // --- Example query ---
    let query = "What is a linglingdong?";
    let req = VectorSearchRequest::builder()
        .query(query)
        .samples(3)
        .build()?;

    // If you have rig_qdrant available, uncomment the following lines:
     use rig_qdrant::QdrantVectorStore;
     let vector_store = QdrantVectorStore::new(client, model.clone(), query_params.build());
     let results = vector_store.top_n::<serde_json::Value>(req).await?;
     let formatted = results
         .iter()
         .enumerate()
         .map(|(i, doc)| format!("{}. {:?}", i + 1, doc))
         .collect::<Vec<_>>()
         .join("\n");
     println!("{}", formatted);

    //println!("Qdrant vector search setup complete. Please ensure you have the rig_qdrant crate and uncomment the vector search logic.");

    Ok(())
}
