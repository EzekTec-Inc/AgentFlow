
---
**Timestamp:** 2026-03-08T04:44:00Z
**Summary:** Added a security warning to the `create_tool_node` function documentation.
**Files Modified:**
- `src/utils/tool.rs`
**Reason:** The function allows for arbitrary shell command execution, which is a significant security risk if used with untrusted input (e.g., from an LLM). The documentation needs to explicitly warn developers about the risk of command injection and recommend mitigation strategies.
**Previous Behavior:** The function documentation described its purpose but did not contain any security warnings.
**New Behavior:** The function documentation now includes a prominent `# Security Warning` section detailing the risks of command injection and advising the use of allow-lists, input sanitization, and sandboxing.
**Rollback Instructions:**
1. Open `src/utils/tool.rs`.
2. Replace the updated documentation block for `create_tool_node` with the original version:
   ```rust
   /// Creates a node that executes an external shell command or script.
   ///
   /// This is used to implement "Tools" (the "Hands" in the Brain-Tool-Context architecture).
   /// Instead of writing a complex trait, we simply wrap an OS-level command
   /// in a standard AgentFlow node.
   ///
   /// The command will be executed asynchronously. The standard output (stdout)
   /// and standard error (stderr) will be captured and placed into the SharedStore
   /// under the keys `{tool_name}_stdout` and `{tool_name}_stderr`. The exit status
   /// is stored under `{tool_name}_status`.
   ```
