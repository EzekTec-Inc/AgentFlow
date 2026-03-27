use std::sync::Arc;
use tokio::sync::RwLock;

/// A strongly-typed, async-safe state container for use with [`TypedFlow`].
///
/// `TypedStore<T>` wraps any `T: Send + Sync` in an `Arc<RwLock<T>>`, giving
/// you compile-time type safety without the runtime overhead of
/// `HashMap<String, Value>` casts.
///
/// Cloning a `TypedStore` clones the `Arc` — both instances share the same
/// underlying state. This matches the semantics of [`SharedStore`].
///
/// # Truncation flag
///
/// If [`TypedFlow::run`] halts because `max_steps` was reached, it sets
/// [`limit_exceeded`](Self::limit_exceeded) to `true` on the returned store.
/// Callers that need strict enforcement should use
/// [`TypedFlow::run_safe`] instead, which returns
/// `Err(AgentFlowError::ExecutionLimitExceeded)`.
///
/// # When to use `TypedStore` vs `SharedStore`
///
/// | | `SharedStore` | `TypedStore<T>` |
/// |---|---|---|
/// | Key access | `store["key"]` (runtime cast) | `store.inner.read().await.field` (compile-time) |
/// | Flexibility | Any JSON-serialisable data | Fixed struct `T` |
/// | Routing | `"action"` key in store | Closure returning `Option<String>` |
/// | Recommended for | Dynamic / ad-hoc pipelines | Strict state machines |
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::core::typed_store::TypedStore;
///
/// #[derive(Debug, Clone)]
/// struct MyState { count: u32 }
///
/// #[tokio::main]
/// async fn main() {
///     let store = TypedStore::new(MyState { count: 0 });
///     store.inner.write().await.count += 1;
///     assert_eq!(store.inner.read().await.count, 1);
/// }
/// ```
///
/// [`TypedFlow`]: crate::core::typed_flow::TypedFlow
/// [`TypedFlow::run`]: crate::core::typed_flow::TypedFlow::run
/// [`TypedFlow::run_safe`]: crate::core::typed_flow::TypedFlow::run_safe
/// [`SharedStore`]: crate::core::node::SharedStore
#[derive(Debug)]
pub struct TypedStore<T> {
    /// The inner `Arc<RwLock<T>>`. Access state via `.read().await` /
    /// `.write().await`.
    pub inner: T,

    /// Set to `true` by [`TypedFlow::run`] when execution was halted because
    /// `max_steps` was reached. Always `false` for a freshly created store.
    ///
    /// Use this flag to detect silent truncation when using [`TypedFlow::run`].
    /// For strict enforcement, use [`TypedFlow::run_safe`] instead.
    ///
    /// [`TypedFlow::run`]: crate::core::typed_flow::TypedFlow::run
    /// [`TypedFlow::run_safe`]: crate::core::typed_flow::TypedFlow::run_safe
    pub limit_exceeded: bool,
}

impl<T> TypedStore<T> {
    /// Create a new `TypedStore` wrapping `state`.
    pub fn new(state: T) -> Self {
        Self {
            inner: state,
            limit_exceeded: false,
        }
    }

    /// Returns `true` if [`TypedFlow::run`] halted this store due to
    /// `max_steps` being exceeded.
    ///
    /// [`TypedFlow::run`]: crate::core::typed_flow::TypedFlow::run
    pub fn limit_exceeded(&self) -> bool {
        self.limit_exceeded
    }
}

impl<T: Clone> Clone for TypedStore<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            limit_exceeded: self.limit_exceeded,
        }
    }
}
