use crate::core::node::{create_node, SharedStore, SimpleNode};
use serde_json::Value;
use tokio::process::Command;

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
///
/// # Security Warning
///
/// This function can execute **any** shell command. If the `command` or `args` parameters
/// can be influenced by external input (e.g., from an LLM), it can create a
/// **command injection vulnerability**.
///
/// **Do not** pass untrusted input directly to this function. Always use an allow-list
/// for permitted commands and sanitize all arguments. It is highly recommended to run
/// any agent using this node in a sandboxed environment (e.g., a Docker container)
/// to limit potential damage.
pub fn create_tool_node(
    tool_name: impl Into<String>,
    command: impl Into<String>,
    args: Vec<String>,
) -> SimpleNode {
    let tool_name = tool_name.into();
    let command = command.into();

    create_node(move |store: SharedStore| {
        let tool_name = tool_name.clone();
        let command = command.clone();
        let args = args.clone();

        Box::pin(async move {
            let mut cmd = Command::new(&command);
            cmd.args(&args);

            // Execute the command and capture output
            match cmd.output().await {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let status = output.status.code().unwrap_or(-1);

                    let mut guard = store.lock().await;
                    guard.insert(format!("{}_stdout", tool_name), Value::String(stdout));
                    guard.insert(format!("{}_stderr", tool_name), Value::String(stderr));
                    guard.insert(
                        format!("{}_status", tool_name),
                        Value::Number(status.into()),
                    );
                }
                Err(e) => {
                    let mut guard = store.lock().await;
                    guard.insert(
                        format!("{}_error", tool_name),
                        Value::String(format!("Failed to execute tool '{}': {}", command, e)),
                    );
                    guard.insert(format!("{}_status", tool_name), Value::Number((-1).into()));
                }
            }
            store
        })
    })
}
