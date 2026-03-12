# Example: document_processing

*This documentation is automatically generated from the source code.*

# Example: document_processing.rs

Real-world document processing pipeline. The workflow:

1. **Classify** — detects document type (image vs text) from the file extension
2. **Extract** — LLM extracts named entities from the document content
3. **Analyze** — LLM assesses extraction quality and determines semantic context
4. **Retry** — re-runs extraction up to 3 times if the LLM deems quality poor
5. **Convert** — runs a real shell tool (`pandoc` for text, `convert` for images)
   loaded dynamically from `SKILL_DOC_PROCESS.md`
6. **End** — prints a summary

Domain: contract / business document processing.

Requires: OPENAI_API_KEY
Optional: pandoc, imagemagick (falls back to echo mock if not installed)
Run with: cargo run --example document-processing

## Implementation Architecture

```mermaid
graph TD
    Start[(Document File)] --> Classify[Classify Node<br>Determine type]
    Classify --> Extract[Extract Node<br>LLM Entities]
    Extract --> Analyze[Analyze Node<br>LLM Context & Quality]
    Analyze -->|Quality Poor| Retry{Retry limit?}
    Retry -->|No| Extract
    Analyze -->|Quality Good| Convert[Convert Tool Node<br>Pandoc/Imagemagick]
    Convert --> End[(Final Store)]
    
    classDef node fill:#e8f5e9,stroke:#2e7d32,stroke-width:2px;
    classDef tool fill:#fff3e0,stroke:#ef6c00,stroke-width:2px;
    class Classify,Extract,Analyze node;
    class Convert tool;
```

