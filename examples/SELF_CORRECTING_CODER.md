# Example: self_correcting_coder

*This documentation is derived from the source code.*

# Example: self_correcting_coder.rs

**Purpose:**
Demonstrates a resilient, self-healing agent loop that writes code, compiles it, and uses compiler errors to automatically correct mistakes via built-in retry logic.

**How it works:**
- **Generator Node:** An agent writes a Rust function based on a prompt. It reads any previous compilation errors from the store to adjust its code.
- **Evaluator Node:** The generated code is written to the system's temporary directory and compiled using `rustc` via a system command. If compilation fails, the stderr is saved back to the store under the `"error"` key.
- **Automatic Retries:** The entire sub-flow is wrapped in an `Agent::with_retry`. The framework automatically detects the presence of the `"error"` key in the output and re-triggers the flow, passing the error back to the Generator node up to the `max_retries` limit.

**How to adapt:**
- Use this pattern for code generation, automated data formatting, SQL query generation, or any scenario where output can be programmatically validated (e.g., via a compiler, linter, or schema validator) and fed back to the LLM for self-correction.

**Example execution:**
```bash
cargo run --example self_correcting_coder
```
