use crate::core::error::AgentFlowError;
use crate::core::node::{create_node, SharedStore, SimpleNode};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::{Child, Command};
use tracing::{debug, warn};

/// Default timeout for external tool execution (30 seconds).
pub const DEFAULT_TOOL_TIMEOUT: Duration = Duration::from_secs(30);

async fn wait_with_timeout(
    child: Child,
    timeout: Duration,
) -> Result<std::process::Output, std::io::Error> {
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("process timed out after {}s", timeout.as_secs()),
        )),
    }
}

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
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            cmd.kill_on_drop(true);

            let result = match cmd.spawn() {
                Ok(child) => wait_with_timeout(child, timeout).await,
                Err(e) => Err(e),
            };

            match result {
                Ok(output) => {
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
                Err(e) if e.kind() != std::io::ErrorKind::TimedOut => {
                    let mut guard = store.write().await;
                    guard.insert(
                        format!("{}_error", tool_name),
                        Value::String(format!("Failed to execute tool '{}': {}", command, e)),
                    );
                    guard.insert(format!("{}_status", tool_name), Value::Number((-1).into()));
                }
                Err(_) => {
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

// ── ToolRegistry ─────────────────────────────────────────────────────────────

/// A registered tool entry: the OS binary path and its fixed argument list.
#[derive(Debug, Clone)]
pub struct ToolEntry {
    /// The binary to invoke (e.g. `"uname"`, `"/usr/bin/curl"`).
    pub command: String,
    /// Fixed arguments prepended before any runtime arguments.
    pub args: Vec<String>,
    /// Per-tool timeout, falling back to [`DEFAULT_TOOL_TIMEOUT`] when `None`.
    pub timeout: Option<Duration>,
}

/// An explicit allowlist of tools that agents are permitted to invoke.
///
/// `ToolRegistry` is the recommended security boundary between LLM-generated
/// tool names and actual OS process spawning.  Only tools that have been
/// explicitly [`register`](Self::register)ed can be executed; any unrecognised
/// name returns an error node that writes `{name}_error` into the store.
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::utils::tool::ToolRegistry;
///
/// let mut registry = ToolRegistry::new();
/// registry.register("sysinfo",  "uname",  vec!["-a".into()], None);
/// registry.register("hostname", "hostname", vec![],          None);
///
/// // Later, inside a node or flow:
/// let node = registry.create_node("sysinfo").unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    tools: HashMap<String, ToolEntry>,
}

impl ToolRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool under `name`.
    ///
    /// - `command` — the binary to run.
    /// - `args`    — fixed argument list (appended to the command, before any
    ///   runtime args).
    /// - `timeout` — optional per-tool timeout; falls back to
    ///   [`DEFAULT_TOOL_TIMEOUT`] when `None`.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        command: impl Into<String>,
        args: Vec<String>,
        timeout: Option<Duration>,
    ) {
        self.tools.insert(
            name.into(),
            ToolEntry {
                command: command.into(),
                args,
                timeout,
            },
        );
    }

    /// Return `true` if `name` is in the allowlist.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Look up the [`ToolEntry`] for `name`.
    pub fn get(&self, name: &str) -> Option<&ToolEntry> {
        self.tools.get(name)
    }

    /// Create a [`SimpleNode`] for the registered tool `name`.
    ///
    /// # Errors
    ///
    /// Returns `Err(AgentFlowError::NotFound)` if `name` is not registered.
    pub fn create_node(&self, name: &str) -> Result<SimpleNode, AgentFlowError> {
        let entry = self.tools.get(name).ok_or_else(|| {
            AgentFlowError::NotFound(format!(
                "Tool '{}' is not registered in the ToolRegistry. \
                 Registered tools: [{}]",
                name,
                self.tools.keys().cloned().collect::<Vec<_>>().join(", ")
            ))
        })?;

        let timeout = entry.timeout.unwrap_or(DEFAULT_TOOL_TIMEOUT);
        Ok(create_tool_node_with_timeout(
            name,
            &entry.command,
            entry.args.clone(),
            timeout,
        ))
    }

    /// Wrap the registry in an [`Arc`] for cheap sharing across threads/tasks.
    pub fn into_arc(self) -> Arc<Self> {
        Arc::new(self)
    }
}

// ── Corrective-retry node ─────────────────────────────────────────────────────

/// Create a [`SimpleNode`] that retries an async operation and feeds the
/// **failure reason back into the store** on each retry attempt.
///
/// This implements a *self-correction loop*: instead of blindly re-running the
/// same prompt, each retry writes the previous error message into the store
/// under `{error_key}` so that downstream nodes (typically an LLM prompt
/// builder) can read it and adjust their output.
///
/// # Arguments
///
/// - `exec`      — the async operation to attempt.  Receives the current
///   [`SharedStore`] and returns `Result<SharedStore, AgentFlowError>`.
/// - `max_retries` — maximum number of attempts (1 = no retries).
/// - `wait_millis` — delay between attempts in milliseconds.
/// - `error_key`   — store key under which the last error message is written
///   before each retry (e.g. `"last_error"`).
///
/// # Behaviour
///
/// 1. Run `exec` with the current store.
/// 2. On `Ok` — return immediately.
/// 3. On `Err` — write `error_key = "<error message>"` into the store, wait
///    `wait_millis` ms, and repeat up to `max_retries` times total.
/// 4. If all attempts fail, write the final error into `error_key` and return
///    the store unchanged (infallible surface — use
///    `create_result_node` for an `Err` return).
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::utils::tool::create_corrective_retry_node;
/// use agentflow::core::error::AgentFlowError;
///
/// let node = create_corrective_retry_node(
///     |store| async move {
///         // Read the previous error (if any) so the LLM can self-correct.
///         let hint = store.read().await
///             .get("last_error")
///             .and_then(|v| v.as_str())
///             .unwrap_or("")
///             .to_string();
///
///         // ... build prompt incorporating `hint`, call LLM, validate ...
///         // Return Err to trigger another retry:
///         Err(AgentFlowError::NodeFailure("invalid JSON in LLM reply".into()))
///     },
///     3,       // max attempts
///     500,     // ms between retries
///     "last_error",
/// );
/// ```
pub fn create_corrective_retry_node<F, Fut>(
    exec: F,
    max_retries: usize,
    wait_millis: u64,
    error_key: impl Into<String>,
) -> SimpleNode
where
    F: Fn(SharedStore) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = Result<SharedStore, AgentFlowError>> + Send + 'static,
{
    let error_key = error_key.into();

    create_node(move |store: SharedStore| {
        let exec = exec.clone();
        let error_key = error_key.clone();

        async move {
            let current_store = store;
            let retries = max_retries.max(1);

            for attempt in 0..retries {
                debug!(attempt, max_retries = retries, "corrective_retry attempt");
                match exec(current_store.clone()).await {
                    Ok(s) => {
                        debug!(attempt, "corrective_retry succeeded");
                        // Clear any lingering error key from a previous attempt.
                        current_store.write().await.remove(&error_key);
                        return s;
                    }
                    Err(e) => {
                        warn!(attempt, error = %e, "corrective_retry failed; injecting error into store");
                        current_store
                            .write()
                            .await
                            .insert(error_key.clone(), Value::String(e.to_string()));

                        if attempt < retries - 1 && wait_millis > 0 {
                            tokio::time::sleep(Duration::from_millis(wait_millis)).await;
                        }
                    }
                }
            }

            // All retries exhausted — store already contains the last error.
            current_store
        }
    })
}
