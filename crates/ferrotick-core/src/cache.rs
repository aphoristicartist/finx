//! In-memory caching for API responses.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Defines the behavior of the in-memory cache for an API call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheMode {
    /// Read from the cache if a non-expired entry is present;
    /// otherwise, fetch from the network and write the response to the cache. (Default)
    Use,
    /// Always fetch from the network, bypassing any cached entry,
    /// and write the new response to the cache.
    Refresh,
    /// Always fetch from the network and do not read from or write to the cache.
    Bypass,
}

impl Default for CacheMode {
    fn default() -> Self {
        Self::Use
    }
}

#[derive(Debug, Clone)]
struct CacheEntry {
    body: String,
    expires_at: Instant,
}

#[derive(Debug)]
struct CacheInner {
    map: HashMap<String, CacheEntry>,
    default_ttl: Duration,
}

impl CacheInner {
    fn new(default_ttl: Duration) -> Self {
        Self {
            map: HashMap::new(),
            default_ttl,
        }
    }

    fn get(&self, key: &str) -> Option<String> {
        self.map.get(key).and_then(|entry| {
            if Instant::now() <= entry.expires_at {
                Some(entry.body.clone())
            } else {
                None
            }
        })
    }

    fn put(&mut self, key: String, body: String, ttl_override: Option<Duration>) {
        let ttl = ttl_override.unwrap_or(self.default_ttl);
        let expires_at = Instant::now() + ttl;
        self.map.insert(key, CacheEntry { body, expires_at });
    }

    fn clear_expired(&mut self) {
        let now = Instant::now();
        self.map.retain(|_, entry| entry.expires_at > now);
    }

    fn clear(&mut self) {
        self.map.clear();
    }

    fn len(&self) -> usize {
        self.map.len()
    }
}

/// Thread-safe in-memory cache for API responses.
#[derive(Debug, Clone)]
pub struct CacheStore {
    inner: Arc<tokio::sync::RwLock<CacheInner>>,
}

impl CacheStore {
    /// Create a new cache store with a default TTL.
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            inner: Arc::new(tokio::sync::RwLock::new(CacheInner::new(default_ttl))),
        }
    }

    /// Create a cache store with a default TTL of 5 minutes.
    pub fn with_default_ttl() -> Self {
        Self::new(Duration::from_secs(300))
    }

    /// Create a disabled cache (Bypass mode for all requests).
    pub fn disabled() -> Self {
        Self::new(Duration::ZERO)
    }

    /// Get a cached value for the given key if it exists and hasn't expired.
    ///
    /// Returns `None` if:
    /// - No entry exists for the key
    /// - The entry has expired
    /// - The cache is disabled (TTL is ZERO)
    pub async fn get(&self, key: &str) -> Option<String> {
        let store = self.inner.read().await;
        store.get(key)
    }

    /// Put a value into the cache with the given key.
    ///
    /// If `ttl_override` is provided, it will be used instead of the default TTL.
    /// If the cache is disabled (TTL is ZERO), this is a no-op.
    pub async fn put(&self, key: String, body: String, ttl_override: Option<Duration>) {
        let mut store = self.inner.write().await;

        // Don't put anything if cache is disabled
        if store.default_ttl == Duration::ZERO {
            return;
        }

        store.put(key, body, ttl_override);
    }

    /// Remove expired entries from the cache.
    pub async fn clear_expired(&self) {
        let mut store = self.inner.write().await;
        store.clear_expired();
    }

    /// Clear all entries from the cache.
    pub async fn clear(&self) {
        let mut store = self.inner.write().await;
        store.clear();
    }

    /// Get the number of entries in the cache (including expired entries).
    pub async fn len(&self) -> usize {
        let store = self.inner.read().await;
        store.len()
    }

    /// Check if the cache is disabled (TTL is ZERO).
    pub async fn is_disabled(&self) -> bool {
        let store = self.inner.read().await;
        store.default_ttl == Duration::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_store_basic_operations() {
        let cache = CacheStore::new(Duration::from_secs(1));

        // Cache miss
        assert!(cache.get("key1").await.is_none());

        // Put and get
        cache.put("key1".to_string(), "value1".to_string(), None).await;
        assert_eq!(cache.get("key1").await, Some("value1".to_string()));

        // Overwrite
        cache.put("key1".to_string(), "value2".to_string(), None).await;
        assert_eq!(cache.get("key1").await, Some("value2".to_string()));
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let cache = CacheStore::new(Duration::from_millis(100));

        cache.put("key1".to_string(), "value1".to_string(), None).await;
        assert!(cache.get("key1").await.is_some());

        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be expired
        assert!(cache.get("key1").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_ttl_override() {
        let cache = CacheStore::new(Duration::from_secs(60));

        cache
            .put(
                "key1".to_string(),
                "value1".to_string(),
                Some(Duration::from_millis(100)),
            )
            .await;

        assert!(cache.get("key1").await.is_some());
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(cache.get("key1").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_clear_expired() {
        let cache = CacheStore::new(Duration::from_millis(100));

        cache.put("key1".to_string(), "value1".to_string(), None).await;
        cache.put("key2".to_string(), "value2".to_string(), None).await;

        assert_eq!(cache.len().await, 2);

        tokio::time::sleep(Duration::from_millis(150)).await;
        cache.clear_expired().await;

        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_cache_clear_all() {
        let cache = CacheStore::new(Duration::from_secs(60));

        cache.put("key1".to_string(), "value1".to_string(), None).await;
        cache.put("key2".to_string(), "value2".to_string(), None).await;

        assert_eq!(cache.len().await, 2);
        cache.clear().await;
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_cache_disabled() {
        let cache = CacheStore::disabled();

        assert!(cache.is_disabled().await);

        cache.put("key1".to_string(), "value1".to_string(), None).await;
        assert!(cache.get("key1").await.is_none());
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_cache_mode_default() {
        let mode: CacheMode = Default::default();
        assert_eq!(mode, CacheMode::Use);
    }
}
