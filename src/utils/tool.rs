use crate::core::node::{create_node, SharedStore, SimpleNode};
use serde_json::Value;
use std::time::Duration;
use tokio::process::Command;

/// Default timeout for external tool execution (30 seconds).
pub const DEFAULT_TOOL_TIMEOUT: Duration = Duration::from_secs(30);

/// Creates a node that executes an external shell command or script.
///
/// This is used to implement "Tools" (the "Hands" in the Brain-Tool-Context architecture).
/// Instead of writing a complex trait, we simply wrap an OS-level command
/// in a standard AgentFlow node.
///
/// The command will be executed asynchronously with a configurable timeout
/// (default: 30 seconds). The standard output (stdout) and standard error (stderr)
/// will be captured and placed into the SharedStore under the keys
/// `{tool_name}_stdout` and `{tool_name}_stderr`. The exit status is stored
/// under `{tool_name}_status`.
///
/// # Timeout
///
/// Use [`create_tool_node_with_timeout`] to specify a custom timeout. If the
/// command does not complete within the timeout, the node writes a timeout error
/// into the store under `{tool_name}_error` with status `-1`.
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
    create_tool_node_with_timeout(tool_name, command, args, DEFAULT_TOOL_TIMEOUT)
}

/// Like [`create_tool_node`] but with an explicit timeout duration.
pub fn create_tool_node_with_timeout(
    tool_name: impl Into<String>,
    command: impl Into<String>,
    args: Vec<String>,
    timeout: Duration,
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

            let result = tokio::time::timeout(timeout, cmd.output()).await;

            match result {
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let status = output.status.code().unwrap_or(-1);

                    let mut guard = store.write().await;
                    guard.insert(format!("{}_stdout", tool_name), Value::String(stdout));
                    guard.insert(format!("{}_stderr", tool_name), Value::String(stderr));
                    guard.insert(
                        format!("{}_status", tool_name),
                        Value::Number(status.into()),
                    );
                }
                Ok(Err(e)) => {
                    let mut guard = store.write().await;
                    guard.insert(
                        format!("{}_error", tool_name),
                        Value::String(format!("Failed to execute tool '{}': {}", command, e)),
                    );
                    guard.insert(format!("{}_status", tool_name), Value::Number((-1).into()));
                }
                Err(_elapsed) => {
                    let mut guard = store.write().await;
                    guard.insert(
                        format!("{}_error", tool_name),
                        Value::String(format!(
                            "Tool '{}' timed out after {}s",
                            command,
                            timeout.as_secs()
                        )),
                    );
                    guard.insert(format!("{}_status", tool_name), Value::Number((-1).into()));
                }
            }
            store
        })
    })
}
