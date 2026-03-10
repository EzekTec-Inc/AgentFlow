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
/// [`SharedStore`]: crate::core::node::SharedStore
#[derive(Debug)]
pub struct TypedStore<T> {
    /// The inner `Arc<RwLock<T>>`. Access state via `.read().await` /
    /// `.write().await`.
    pub inner: Arc<RwLock<T>>,
}

impl<T> TypedStore<T> {
    /// Create a new `TypedStore` wrapping `state`.
    pub fn new(state: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(state)),
        }
    }
}

impl<T> Clone for TypedStore<T> {
    /// Clones the `Arc` — both instances share the same underlying `T`.
    /// `T` does not need to implement `Clone`.
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
