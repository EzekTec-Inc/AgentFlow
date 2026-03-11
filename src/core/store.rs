use crate::core::node::SharedStore;
use serde_json::Value;
use std::collections::HashMap;

/// Typed helper wrapper around a [`SharedStore`].
///
/// `Store` provides ergonomic, type-safe get/set/require methods so you don't
/// have to manually cast `serde_json::Value` in every node.
///
/// It wraps the same `Arc<RwLock<…>>` as [`SharedStore`] and can be freely
/// converted to/from one — use whichever API is more convenient at each call
/// site.
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::core::store::Store;
///
/// #[tokio::main]
/// async fn main() {
///     let store = Store::new();
///     store.set_string("name", "Alice").await;
///     store.set_i64("age", 30).await;
///
///     let name = store.get_string("name").await; // Some("Alice")
///     let age  = store.get_i64("age").await;     // Some(30)
///
///     // Panics with a clear message if the key is absent or the wrong type
///     let name: String = store.require_string("name").await.unwrap();
/// }
/// ```
pub struct Store {
    inner: SharedStore,
}

impl Store {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self {
            inner: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Wrap an existing [`SharedStore`] in a `Store`.
    pub fn from_shared(store: SharedStore) -> Self {
        Self { inner: store }
    }

    /// Consume `self` and return the underlying [`SharedStore`].
    pub fn into_shared(self) -> SharedStore {
        self.inner
    }

    /// Borrow the underlying [`SharedStore`] without consuming `self`.
    pub fn as_shared(&self) -> &SharedStore {
        &self.inner
    }

    /// Get the value at `key` as a `String`, or `None` if absent or not a string.
    pub async fn get_string(&self, key: &str) -> Option<String> {
        let guard = self.inner.read().await;
        guard
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Get the value at `key` as an `i64`, or `None` if absent or not an integer.
    pub async fn get_i64(&self, key: &str) -> Option<i64> {
        let guard = self.inner.read().await;
        guard.get(key).and_then(|v| v.as_i64())
    }

    /// Get the value at `key` as an `f64`, or `None` if absent or not a float.
    pub async fn get_f64(&self, key: &str) -> Option<f64> {
        let guard = self.inner.read().await;
        guard.get(key).and_then(|v| v.as_f64())
    }

    /// Get the value at `key` as a `bool`, or `None` if absent or not a boolean.
    pub async fn get_bool(&self, key: &str) -> Option<bool> {
        let guard = self.inner.read().await;
        guard.get(key).and_then(|v| v.as_bool())
    }

    /// Get a clone of the raw [`Value`] at `key`, or `None` if absent.
    pub async fn get(&self, key: &str) -> Option<Value> {
        let guard = self.inner.read().await;
        guard.get(key).cloned()
    }

    /// Insert a string value.
    pub async fn set_string(&self, key: impl Into<String>, value: impl Into<String>) {
        let mut guard = self.inner.write().await;
        guard.insert(key.into(), Value::String(value.into()));
    }

    /// Insert an integer value.
    pub async fn set_i64(&self, key: impl Into<String>, value: i64) {
        let mut guard = self.inner.write().await;
        guard.insert(key.into(), Value::Number(value.into()));
    }

    /// Insert a float value. Silently does nothing if `value` is NaN or infinite.
    pub async fn set_f64(&self, key: impl Into<String>, value: f64) {
        let mut guard = self.inner.write().await;
        if let Some(num) = serde_json::Number::from_f64(value) {
            guard.insert(key.into(), Value::Number(num));
        }
    }

    /// Insert a boolean value.
    pub async fn set_bool(&self, key: impl Into<String>, value: bool) {
        let mut guard = self.inner.write().await;
        guard.insert(key.into(), Value::Bool(value));
    }

    /// Insert a raw [`Value`].
    pub async fn set(&self, key: impl Into<String>, value: Value) {
        let mut guard = self.inner.write().await;
        guard.insert(key.into(), value);
    }

    /// Get the raw [`Value`] at `key`, or `Err` if absent.
    pub async fn require(&self, key: &str) -> Result<Value, String> {
        let guard = self.inner.read().await;
        guard
            .get(key)
            .cloned()
            .ok_or_else(|| format!("Required key '{}' not found", key))
    }

    /// Get the value at `key` as a `String`, or `Err` if absent or not a string.
    pub async fn require_string(&self, key: &str) -> Result<String, String> {
        self.get_string(key)
            .await
            .ok_or_else(|| format!("Required string key '{}' not found", key))
    }

    /// Get the value at `key` as an `i64`, or `Err` if absent or not an integer.
    pub async fn require_i64(&self, key: &str) -> Result<i64, String> {
        self.get_i64(key)
            .await
            .ok_or_else(|| format!("Required i64 key '{}' not found", key))
    }

    /// Get the value at `key` as an `f64`, or `Err` if absent or not a float.
    pub async fn require_f64(&self, key: &str) -> Result<f64, String> {
        self.get_f64(key)
            .await
            .ok_or_else(|| format!("Required f64 key '{}' not found", key))
    }

    /// Get the value at `key` as a `bool`, or `Err` if absent or not a boolean.
    pub async fn require_bool(&self, key: &str) -> Result<bool, String> {
        self.get_bool(key)
            .await
            .ok_or_else(|| format!("Required bool key '{}' not found", key))
    }

    /// Returns `true` if the store contains `key`.
    pub async fn contains_key(&self, key: &str) -> bool {
        let guard = self.inner.read().await;
        guard.contains_key(key)
    }

    /// Remove `key` from the store, returning its value if it was present.
    pub async fn remove(&self, key: &str) -> Option<Value> {
        let mut guard = self.inner.write().await;
        guard.remove(key)
    }

    /// Remove all entries from the store.
    pub async fn clear(&self) {
        let mut guard = self.inner.write().await;
        guard.clear();
    }

    /// Return all keys currently in the store.
    pub async fn keys(&self) -> Vec<String> {
        let guard = self.inner.read().await;
        guard.keys().cloned().collect()
    }

    /// Return the number of entries in the store.
    pub async fn len(&self) -> usize {
        let guard = self.inner.read().await;
        guard.len()
    }

    /// Return `true` if the store contains no entries.
    pub async fn is_empty(&self) -> bool {
        let guard = self.inner.read().await;
        guard.is_empty()
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Store {
    /// Cloning a `Store` clones the `Arc` — both instances share the same
    /// underlying data.
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
