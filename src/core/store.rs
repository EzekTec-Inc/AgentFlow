use crate::core::node::SharedStore;
use serde_json::Value;
use std::collections::HashMap;

pub struct Store {
    inner: SharedStore,
}

impl Store {
    pub fn new() -> Self {
        Self {
            inner: std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn from_shared(store: SharedStore) -> Self {
        Self { inner: store }
    }

    pub fn into_shared(self) -> SharedStore {
        self.inner
    }

    pub fn as_shared(&self) -> &SharedStore {
        &self.inner
    }

    pub async fn get_string(&self, key: &str) -> Option<String> {
        let guard = self.inner.lock().await;
        guard
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    pub async fn get_i64(&self, key: &str) -> Option<i64> {
        let guard = self.inner.lock().await;
        guard.get(key).and_then(|v| v.as_i64())
    }

    pub async fn get_f64(&self, key: &str) -> Option<f64> {
        let guard = self.inner.lock().await;
        guard.get(key).and_then(|v| v.as_f64())
    }

    pub async fn get_bool(&self, key: &str) -> Option<bool> {
        let guard = self.inner.lock().await;
        guard.get(key).and_then(|v| v.as_bool())
    }

    pub async fn get(&self, key: &str) -> Option<Value> {
        let guard = self.inner.lock().await;
        guard.get(key).cloned()
    }

    pub async fn set_string(&self, key: impl Into<String>, value: impl Into<String>) {
        let mut guard = self.inner.lock().await;
        guard.insert(key.into(), Value::String(value.into()));
    }

    pub async fn set_i64(&self, key: impl Into<String>, value: i64) {
        let mut guard = self.inner.lock().await;
        guard.insert(key.into(), Value::Number(value.into()));
    }

    pub async fn set_f64(&self, key: impl Into<String>, value: f64) {
        let mut guard = self.inner.lock().await;
        if let Some(num) = serde_json::Number::from_f64(value) {
            guard.insert(key.into(), Value::Number(num));
        }
    }

    pub async fn set_bool(&self, key: impl Into<String>, value: bool) {
        let mut guard = self.inner.lock().await;
        guard.insert(key.into(), Value::Bool(value));
    }

    pub async fn set(&self, key: impl Into<String>, value: Value) {
        let mut guard = self.inner.lock().await;
        guard.insert(key.into(), value);
    }

    pub async fn require(&self, key: &str) -> Result<Value, String> {
        let guard = self.inner.lock().await;
        guard
            .get(key)
            .cloned()
            .ok_or_else(|| format!("Required key '{}' not found", key))
    }

    pub async fn require_string(&self, key: &str) -> Result<String, String> {
        self.get_string(key)
            .await
            .ok_or_else(|| format!("Required string key '{}' not found", key))
    }

    pub async fn require_i64(&self, key: &str) -> Result<i64, String> {
        self.get_i64(key)
            .await
            .ok_or_else(|| format!("Required i64 key '{}' not found", key))
    }

    pub async fn require_f64(&self, key: &str) -> Result<f64, String> {
        self.get_f64(key)
            .await
            .ok_or_else(|| format!("Required f64 key '{}' not found", key))
    }

    pub async fn require_bool(&self, key: &str) -> Result<bool, String> {
        self.get_bool(key)
            .await
            .ok_or_else(|| format!("Required bool key '{}' not found", key))
    }

    pub async fn contains_key(&self, key: &str) -> bool {
        let guard = self.inner.lock().await;
        guard.contains_key(key)
    }

    pub async fn remove(&self, key: &str) -> Option<Value> {
        let mut guard = self.inner.lock().await;
        guard.remove(key)
    }

    pub async fn clear(&self) {
        let mut guard = self.inner.lock().await;
        guard.clear();
    }

    pub async fn keys(&self) -> Vec<String> {
        let guard = self.inner.lock().await;
        guard.keys().cloned().collect()
    }

    pub async fn len(&self) -> usize {
        let guard = self.inner.lock().await;
        guard.len()
    }

    pub async fn is_empty(&self) -> bool {
        let guard = self.inner.lock().await;
        guard.is_empty()
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Store {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
