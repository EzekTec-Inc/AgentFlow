# Example: orchestrator_with_tools

*This documentation is generated from the source code.*

# Example: orchestrator_with_tools.rs

Real-world orchestrator that delegates to a ReAct sub-agent. The sub-agent
uses a real shell tool (`uname -a`) and passes the result back to the
Orchestrator LLM, which then writes a human-readable system summary.

How it works:
1. Orchestrator (LLM) receives the main task and delegates to the ReAct flow.
2. ReAct Reasoner (LLM) decides to call the `sysinfo` tool.
3. Tool executor runs `uname -a` via the built-in create_tool_node.
4. ReAct Reasoner (LLM) reads the tool output and produces a final answer.
5. Orchestrator (LLM) formats the answer into a polished report.

Requires: OPENAI_API_KEY
Run with: cargo run --example orchestrator-with-tools
