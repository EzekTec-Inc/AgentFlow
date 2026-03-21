# Example: security_auditor

*This documentation is derived from the source code.*

# Example: security_auditor.rs

**Purpose:**
Demonstrates an advanced multi-agent workflow for automated security auditing using `ParallelFlow`, Shell Tools, and Human-in-the-Loop (HITL) patterns.

**How it works:**
- **Crawler:** A `Flow` executes a shell command (`cargo clippy`) via `create_tool_node` to gather static analysis data.
- **Parallel Analysis:** Uses `ParallelFlow` to fan-out the analysis to multiple agents simultaneously. One agent analyzes the clippy output for logic bugs, while another mocks a scan for hardcoded secrets.
- **Merge Strategy:** A custom merge function aggregates the parallel analysis results into a single shared store.
- **Synthesis:** An agent compiles the aggregated findings into a cohesive markdown security report.
- **HITL Review:** A final node pauses the workflow, prints the draft report to the console, and uses `inquire` to ask the user to explicitly "Approve" or "Reject" the report before proceeding.

**How to adapt:**
- Use this pattern for automated PR reviews, vulnerability scanning pipelines, or any scenario requiring parallel data processing followed by human oversight.

**Example execution:**
```bash
cargo run --example security_auditor
```
