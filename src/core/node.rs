use crate::core::error::AgentFlowError;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use dyn_clone::DynClone;

/// Thread-safe, async-aware key-value store shared between all nodes in a flow.
///
/// Defined as `Arc<RwLock<HashMap<String, Value>>>` so it is cheap to clone
/// (cloning the `Arc` shares the same underlying data) and safe to access
/// concurrently from multiple async tasks.
///
/// # Reserved keys
///
/// - `"action"` — read by [`Flow`] after each node to determine the next
///   transition. Nodes should write this key to control routing.
///   It is automatically removed by `Flow` when execution ends.
///
/// # Deadlock prevention
///
/// **Never hold a write guard across an `.await` point.**
///
/// ```rust,no_run
/// # use agentflow::SharedStore;
/// # async fn example(store: SharedStore) {
/// // ✅ correct — drop the guard before awaiting
/// {
///     let mut g = store.write().await;
///     g.insert("key".into(), serde_json::json!("value"));
/// } // guard dropped here
/// some_async_fn().await;
/// # async fn some_async_fn() {}
/// # }
/// ```
///
/// [`Flow`]: crate::core::flow::Flow
pub type SharedStore = Arc<tokio::sync::RwLock<HashMap<String, Value>>>;

/// Core async node trait.
///
/// Every computation unit in AgentFlow implements this trait. It is generic
/// over input `I` and output `O`, but in practice most nodes operate on
/// [`SharedStore`] → [`SharedStore`].
///
/// Implement this directly for advanced use cases; for the common closure-based
/// pattern use [`create_node`] instead.
///
/// # Object safety
///
/// The trait is object-safe and can be stored as `Box<dyn Node<I, O>>`.
/// [`DynClone`] is a supertrait so boxed nodes can be cloned.
///
/// # `'static` requirement
///
/// [`dyn_clone::clone_trait_object!`] expands the blanket `Clone` impl with a
/// `'static` bound, so **every concrete type that implements `Node` must be
/// `'static`**. This means closures (or structs) that capture references with
/// a non-`'static` lifetime will not compile.
///
/// ```rust,compile_fail
/// use agentflow::core::node::create_node;
///
/// let data = String::from("hello");
/// let _node = create_node(|store| {
///     let r = &data; // ← captures a non-`'static` reference
///     async move { store }
/// });
/// ```
///
/// **Workarounds:**
/// - Clone the data into the closure: `let data = data.clone();`
/// - Wrap shared data in `Arc` and move the `Arc` into the closure.
/// - Move owned values directly (the `move` keyword on the outer closure).
pub trait Node<I, O>: Send + Sync + DynClone {
    /// Execute the node with `input`, returning a future that resolves to `O`.
    fn call(&self, input: I) -> Pin<Box<dyn Future<Output = O> + Send + '_>>;
}
dyn_clone::clone_trait_object!(<I, O> Node<I, O>);

/// Fallible variant of [`Node`] whose output is `Result<O, AgentFlowError>`.
///
/// Use this when a node can legitimately fail (e.g. an LLM call that may time
/// out, or a parser that may receive malformed input). Combine with
/// [`Agent::decide_result`] for automatic retry on [`AgentFlowError::Timeout`].
///
/// Create instances with [`create_result_node`].
///
/// # `'static` requirement
///
/// Same constraint as [`Node`]: every concrete implementation must be
/// `'static`. Closures capturing non-`'static` references will not compile.
/// See [`Node`]'s documentation for workarounds.
///
/// [`Agent::decide_result`]: crate::patterns::agent::Agent::decide_result
pub trait NodeResult<I, O>: Send + Sync + DynClone {
    /// Execute the node, returning `Ok(O)` on success or an [`AgentFlowError`] on failure.
    fn call(
        &self,
        input: I,
    ) -> Pin<Box<dyn Future<Output = Result<O, AgentFlowError>> + Send + '_>>;
}
dyn_clone::clone_trait_object!(<I, O> NodeResult<I, O>);

/// Type alias for a boxed infallible node operating on [`SharedStore`].
///
/// This is the most common node type used throughout AgentFlow patterns.
/// Create one with [`create_node`].
pub type SimpleNode = Box<dyn Node<SharedStore, SharedStore>>;

/// Type alias for a boxed fallible node operating on [`SharedStore`].
///
/// Create one with [`create_result_node`].
pub type ResultNode = Box<dyn NodeResult<SharedStore, SharedStore>>;

/// Create an infallible [`SimpleNode`] from an async closure.
///
/// The closure receives a [`SharedStore`] and must return a future that resolves
/// to a [`SharedStore`]. Routing is controlled by writing `"action"` into the
/// store before returning.
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::prelude::*;
///
/// let node = create_node(|store: SharedStore| async move {
///     store.write().await.insert("result".into(), serde_json::json!("done"));
///     store.write().await.insert("action".into(), serde_json::json!("next"));
///     store
/// });
/// ```
pub fn create_node<F, Fut>(func: F) -> SimpleNode
where
    F: Fn(SharedStore) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = SharedStore> + Send + 'static,
{
    #[derive(Clone)]
    struct FuncNode<F>(F);

    impl<F, Fut> Node<SharedStore, SharedStore> for FuncNode<F>
    where
        F: Fn(SharedStore) -> Fut + Send + Sync + Clone,
        Fut: Future<Output = SharedStore> + Send + 'static,
    {
        fn call(
            &self,
            input: SharedStore,
        ) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
            Box::pin(self.0(input))
        }
    }

    Box::new(FuncNode(func))
}

/// Create a fallible [`ResultNode`] from an async closure.
///
/// The closure receives a [`SharedStore`] and returns
/// `Result<SharedStore, AgentFlowError>`. Use this when the node can fail in a
/// meaningful way (network errors, missing keys, LLM timeouts, etc.).
///
/// Pair with [`Agent::decide_result`] for automatic retry on
/// [`AgentFlowError::Timeout`], or with [`Flow::run_safe`] to halt the flow on
/// any node failure.
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::prelude::*;
/// use agentflow::core::error::AgentFlowError;
///
/// let node = create_result_node(|store: SharedStore| async move {
///     let has_input = store.read().await.contains_key("prompt");
///     if !has_input {
///         return Err(AgentFlowError::NotFound("prompt key missing".into()));
///     }
///     store.write().await.insert("response".into(), serde_json::json!("ok"));
///     Ok(store)
/// });
/// ```
///
/// [`Agent::decide_result`]: crate::patterns::agent::Agent::decide_result
/// [`Flow::run_safe`]: crate::core::flow::Flow::run_safe
pub fn create_result_node<F, Fut>(func: F) -> ResultNode
where
    F: Fn(SharedStore) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = Result<SharedStore, AgentFlowError>> + Send + 'static,
{
    #[derive(Clone)]
    struct ResultFuncNode<F>(F);

    impl<F, Fut> NodeResult<SharedStore, SharedStore> for ResultFuncNode<F>
    where
        F: Fn(SharedStore) -> Fut + Send + Sync + Clone,
        Fut: Future<Output = Result<SharedStore, AgentFlowError>> + Send + 'static,
    {
        fn call(
            &self,
            input: SharedStore,
        ) -> Pin<Box<dyn Future<Output = Result<SharedStore, AgentFlowError>> + Send + '_>>
        {
            Box::pin(self.0(input))
        }
    }

    Box::new(ResultFuncNode(func))
}

// ── StateDiff ────────────────────────────────────────────────────────────────

/// A set of key-value changes a node wants to apply to the [`SharedStore`].
///
/// Using `StateDiff` is the safest way to write node logic because it
/// **eliminates the possibility of holding a write-lock across an `.await`
/// point** — the most common cause of deadlocks in async AgentFlow code.
///
/// # How it works
///
/// Instead of receiving a `SharedStore` and mutating it directly, a
/// *diff node* (created with [`create_diff_node`]) receives a **read-only
/// snapshot** of the current state (`HashMap<String, Value>`), performs its
/// async work (LLM calls, tool invocations, etc.) without touching any lock,
/// and returns a `StateDiff` describing what should change.  The framework
/// applies those changes to the live store after the node's future resolves —
/// guaranteeing the write lock is never held across a suspension point.
///
/// # Routing
///
/// Set the reserved `"action"` key inside the diff to control which edge the
/// [`Flow`] will follow after this node, exactly as with [`create_node`].
///
/// [`Flow`]: crate::core::flow::Flow
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::core::node::{create_diff_node, StateDiff};
/// use serde_json::json;
///
/// let node = create_diff_node(|snapshot| async move {
///     // `snapshot` is a plain HashMap — no lock, no Arc, no await needed.
///     let name = snapshot.get("name")
///         .and_then(|v| v.as_str())
///         .unwrap_or("world");
///
///     // Simulate an async operation (e.g. an LLM call) — no lock held here.
///     // tokio::time::sleep(...).await;
///
///     let mut diff = StateDiff::new();
///     diff.set("greeting", json!(format!("Hello, {name}!")));
///     diff.set("action", json!("next"));
///     diff
/// });
/// ```
#[derive(Debug, Clone, Default)]
pub struct StateDiff {
    changes: HashMap<String, Value>,
}

impl StateDiff {
    /// Create an empty diff.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `key` should be set to `value` in the store.
    pub fn set(&mut self, key: impl Into<String>, value: Value) {
        self.changes.insert(key.into(), value);
    }

    /// Record that `key` should be removed from the store.
    ///
    /// Internally this stores a sentinel `Value::Null` for the key.
    /// The framework interprets `Null` as a deletion instruction.
    pub fn remove(&mut self, key: impl Into<String>) {
        self.changes.insert(key.into(), Value::Null);
    }

    /// Consume the diff and return the inner change map.
    pub fn into_changes(self) -> HashMap<String, Value> {
        self.changes
    }
}

/// Create a [`SimpleNode`] whose logic never touches the [`SharedStore`] lock.
///
/// The closure receives a **read-only snapshot** (`HashMap<String, Value>`) of
/// the current store state.  It returns a [`StateDiff`] that the framework
/// applies atomically after the future resolves.
///
/// - Keys in the diff set to [`Value::Null`] are **deleted** from the store.
/// - All other keys are **upserted** (inserted or overwritten).
///
/// This is the recommended way to write nodes that contain `.await` points,
/// as it makes deadlocks structurally impossible.
///
/// See [`StateDiff`] for a full example.
pub fn create_diff_node<F, Fut>(func: F) -> SimpleNode
where
    F: Fn(HashMap<String, Value>) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = StateDiff> + Send + 'static,
{
    #[derive(Clone)]
    struct DiffNode<F>(F);

    impl<F, Fut> Node<SharedStore, SharedStore> for DiffNode<F>
    where
        F: Fn(HashMap<String, Value>) -> Fut + Send + Sync + Clone,
        Fut: Future<Output = StateDiff> + Send + 'static,
    {
        fn call(
            &self,
            store: SharedStore,
        ) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
            Box::pin(async move {
                // Take a cheap read-only snapshot — lock released immediately.
                let snapshot = store.read().await.clone();

                // Run user logic with NO lock held.
                let diff = self.0(snapshot).await;

                // Apply changes under a single, brief write lock.
                {
                    let mut guard = store.write().await;
                    for (key, value) in diff.into_changes() {
                        if value.is_null() {
                            guard.remove(&key);
                        } else {
                            guard.insert(key, value);
                        }
                    }
                }

                store
            })
        }
    }

    Box::new(DiffNode(func))
}

// ─────────────────────────────────────────────────────────────────────────────

/// Create a [`SimpleNode`] with a built-in prep → exec (with retry) → post pipeline.
///
/// This is useful when you need to separate concerns:
///
/// 1. **`prep`** — extract and validate inputs from the store, returning a
///    `serde_json::Value` payload.
/// 2. **`exec`** — perform the expensive/fallible operation (e.g. LLM call)
///    using the prep payload. Returns `Result<Value, AgentFlowError>`.
///    Retried up to `max_retries` times on failure.
/// 3. **`post`** — write the result back into the store and return it.
///
/// If all retries fail and no `fallback` is provided, a JSON error object is
/// passed to `post`. If a `fallback` function is provided it is called instead.
pub fn create_retry_node<PrepF, PrepFut, ExecF, ExecFut, PostF, PostFut>(
    prep: PrepF,
    exec: ExecF,
    post: PostF,
    max_retries: usize,
    wait_millis: u64,
    fallback: Option<fn(&SharedStore, &Value, &AgentFlowError) -> SharedStore>,
) -> SimpleNode
where
    PrepF: Fn(SharedStore) -> PrepFut + Send + Sync + Clone + 'static,
    PrepFut: Future<Output = Value> + Send + 'static,
    ExecF: Fn(&SharedStore, &Value) -> ExecFut + Send + Sync + Clone + 'static,
    ExecFut: Future<Output = Result<Value, AgentFlowError>> + Send + 'static,
    PostF: Fn(SharedStore, &Value, &Value) -> PostFut + Send + Sync + Clone + 'static,
    PostFut: Future<Output = SharedStore> + Send + 'static,
{
    #[derive(Clone)]
    struct RetryNode<PrepF, ExecF, PostF> {
        prep: PrepF,
        exec: ExecF,
        post: PostF,
        max_retries: usize,
        wait_millis: u64,
        fallback: Option<fn(&SharedStore, &Value, &AgentFlowError) -> SharedStore>,
    }

    impl<PrepF, PrepFut, ExecF, ExecFut, PostF, PostFut> Node<SharedStore, SharedStore>
        for RetryNode<PrepF, ExecF, PostF>
    where
        PrepF: Fn(SharedStore) -> PrepFut + Send + Sync + Clone + 'static,
        PrepFut: Future<Output = Value> + Send + 'static,
        ExecF: Fn(&SharedStore, &Value) -> ExecFut + Send + Sync + Clone + 'static,
        ExecFut: Future<Output = Result<Value, AgentFlowError>> + Send + 'static,
        PostF: Fn(SharedStore, &Value, &Value) -> PostFut + Send + Sync + Clone + 'static,
        PostFut: Future<Output = SharedStore> + Send + 'static,
    {
        fn call(
            &self,
            input: SharedStore,
        ) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
            let prep = self.prep.clone();
            let exec = self.exec.clone();
            let post = self.post.clone();
            let max_retries = self.max_retries;
            let wait_millis = self.wait_millis;
            let fallback = self.fallback;
            Box::pin(async move {
                let prep_res = prep(input.clone()).await;
                let mut last_err: Option<AgentFlowError> = None;
                let mut exec_res: Option<Value> = None;
                for attempt in 0..max_retries {
                    match exec(&input, &prep_res).await {
                        Ok(val) => {
                            exec_res = Some(val);
                            break;
                        }
                        Err(e) => {
                            last_err = Some(e);
                            if attempt + 1 < max_retries && wait_millis > 0 {
                                tokio::time::sleep(Duration::from_millis(wait_millis)).await;
                            }
                        }
                    }
                }
                let exec_val = if let Some(val) = exec_res {
                    val
                } else if let Some(fallback_fn) = fallback {
                    let default_err = AgentFlowError::NodeFailure("no error recorded".into());
                    let _fallback_store =
                        fallback_fn(&input, &prep_res, last_err.as_ref().unwrap_or(&default_err));
                    serde_json::json!({"error": "fallback triggered"})
                } else {
                    serde_json::json!({"error": format!("Node failed after {} retries: {:?}", max_retries, last_err)})
                };
                post(input, &prep_res, &exec_val).await
            })
        }
    }

    Box::new(RetryNode {
        prep,
        exec,
        post,
        max_retries,
        wait_millis,
        fallback,
    })
}

/// Create a node that accepts and returns a `Vec<SharedStore>` batch.
///
/// Useful when you need a single node to process an entire batch in one call,
/// for example a reducer step in a [`MapReduce`] pipeline.
///
/// For processing each item in a batch independently, see [`Batch`] and
/// [`ParallelBatch`].
///
/// [`MapReduce`]: crate::patterns::mapreduce::MapReduce
/// [`Batch`]: crate::core::batch::Batch
/// [`ParallelBatch`]: crate::core::batch::ParallelBatch
pub fn create_batch_node<F, Fut>(func: F) -> Box<dyn Node<Vec<SharedStore>, SharedStore>>
where
    F: Fn(Vec<SharedStore>) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = SharedStore> + Send + 'static,
{
    #[derive(Clone)]
    struct BatchFuncNode<F>(F);

    impl<F, Fut> Node<Vec<SharedStore>, SharedStore> for BatchFuncNode<F>
    where
        F: Fn(Vec<SharedStore>) -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = SharedStore> + Send + 'static,
    {
        fn call(
            &self,
            input: Vec<SharedStore>,
        ) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
            Box::pin(self.0(input))
        }
    }

    Box::new(BatchFuncNode(func))
}

/// Blanket implementation so a `Box<dyn Node<I, O>>` is itself a `Node<I, O>`.
impl<I, O> Node<I, O> for Box<dyn Node<I, O>>
where
    I: Send + 'static,
    O: Send + 'static,
{
    fn call(&self, input: I) -> Pin<Box<dyn Future<Output = O> + Send + '_>> {
        (**self).call(input)
    }
}

/// Blanket implementation so a `Box<dyn NodeResult<I, O>>` is itself a `NodeResult<I, O>`.
impl<I, O> NodeResult<I, O> for Box<dyn NodeResult<I, O>>
where
    I: Send + 'static,
    O: Send + 'static,
{
    fn call(
        &self,
        input: I,
    ) -> Pin<Box<dyn Future<Output = Result<O, AgentFlowError>> + Send + '_>> {
        (**self).call(input)
    }
}
