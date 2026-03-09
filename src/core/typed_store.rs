use std::sync::Arc;
use tokio::sync::RwLock;

/// A strongly-typed alternative to `SharedStore` (which uses `HashMap<String, Value>`).
/// 
/// `TypedStore` wraps any struct that implements `Send + Sync + Clone` in an `Arc<RwLock<T>>`.
/// This provides compile-time type safety for state transitions in flows and agents, 
/// eliminating the need to manually extract, cast, and handle missing keys from a `HashMap`.
#[derive(Debug)]
pub struct TypedStore<T> {
    pub inner: Arc<RwLock<T>>,
}

impl<T> TypedStore<T> {
    /// Create a new `TypedStore` wrapping the provided state.
    pub fn new(state: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(state)),
        }
    }
}

// Implement Clone manually to only clone the Arc, not the underlying T.
// We don't require T to be Clone just to share the Arc.
impl<T> Clone for TypedStore<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
