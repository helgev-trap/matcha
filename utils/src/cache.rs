use crate::rwoption::{RwOption, RwOptionReadGuard, RwOptionWriteGuard};
use std::future::Future;

pub struct Cache<K: PartialEq + Clone, V> {
    data: Option<(K, V)>,
}

impl<K: PartialEq + Clone, V> Default for Cache<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: PartialEq + Clone, V> Cache<K, V> {
    pub fn new() -> Self {
        Cache { data: None }
    }

    pub fn set(&mut self, key: K, value: V) {
        self.data = Some((key, value));
    }

    pub fn clear(&mut self) {
        self.data = None;
    }

    pub fn get(&self) -> Option<(&K, &V)> {
        self.data.as_ref().map(|(k, v)| (k, v))
    }

    pub fn get_mut(&mut self) -> Option<(&K, &mut V)> {
        self.data.as_mut().map(|(k, v)| (k as &K, v))
    }

    pub fn get_or_insert(&mut self, key: &K, value: V) -> (&K, &mut V) {
        self.get_or_insert_with(key, || value)
    }

    pub fn get_or_insert_default(&mut self, key: &K) -> (&K, &mut V)
    where
        V: Default,
    {
        self.get_or_insert_with(key, V::default)
    }

    pub fn get_or_insert_with<F>(&mut self, key: &K, f: F) -> (&K, &mut V)
    where
        F: FnOnce() -> V,
    {
        if !self.get().is_some_and(|(k, _)| k == key) {
            self.set(key.clone(), f());
        }

        self.get_mut()
            .expect("infallible: cache is guaranteed to be populated")
    }

    pub fn get_or_insert_with_eviction_callback<F, G>(
        &mut self,
        key: &K,
        f: F,
        on_eviction: G,
    ) -> (&K, &mut V)
    where
        F: FnOnce() -> V,
        G: FnOnce(K, V),
    {
        if !self.get().is_some_and(|(k, _)| k == key) {
            if let Some((old_key, old_value)) = self.data.take() {
                on_eviction(old_key, old_value);
            }
            self.set(key.clone(), f());
        }

        self.get_mut()
            .expect("infallible: cache is guaranteed to be populated")
    }

    pub async fn get_or_insert_with_async<F>(&mut self, key: K, f: F) -> (&K, &mut V)
    where
        F: Future<Output = V>,
    {
        if !self.get().is_some_and(|(k, _)| *k == key) {
            self.set(key, f.await);
        }

        self.get_mut()
            .expect("infallible: cache is guaranteed to be populated")
    }
}

pub struct RwCache<K: PartialEq + Clone, V> {
    data: RwOption<(K, V)>,
}

impl<K: PartialEq + Clone, V> Default for RwCache<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: PartialEq + Clone, V> RwCache<K, V> {
    pub fn new() -> Self {
        RwCache {
            data: RwOption::new(),
        }
    }

    /// Replace the cached entry with the provided (key, value).
    pub fn set(&self, key: K, value: V) {
        self.data.set((key, value));
    }

    /// Clear the cache (set to None).
    pub fn clear(&self) {
        let _ = self.data.take();
    }

    /// Return a read guard to the cached (K, V) if present.
    pub fn get<'a>(&'a self) -> Option<RwOptionReadGuard<'a, (K, V)>> {
        self.data.get()
    }

    /// Return a write-capable guard to the cached (K, V) if present.
    pub fn get_mut<'a>(&'a self) -> Option<RwOptionWriteGuard<'a, (K, V)>> {
        self.data.get_mut()
    }

    /// Ensure the cache holds the given key; insert `value` if missing or different.
    /// Returns a write guard to the stored (K, V).
    pub fn get_or_insert<'a>(&'a self, key: &K, value: V) -> RwOptionWriteGuard<'a, (K, V)> {
        self.get_or_insert_with(key, || value)
    }

    /// Ensure the cache holds the given key; insert default if missing or different.
    pub fn get_or_insert_default<'a>(&'a self, key: &K) -> RwOptionWriteGuard<'a, (K, V)>
    where
        V: Default,
    {
        self.get_or_insert_with(key, V::default)
    }

    /// Ensure the cache holds the given key; initialize with `f` if missing or different.
    pub fn get_or_insert_with<'a, F>(&'a self, key: &K, f: F) -> RwOptionWriteGuard<'a, (K, V)>
    where
        F: FnOnce() -> V,
    {
        // Fast path: already present with same key
        if !self.data.is_some_and(|(k, _)| k == key) {
            // Evict existing value if any
            if self.data.is_some() {
                let _ = self.data.take();
            }
            self.data.set((key.clone(), f()));
        }

        self.data
            .get_mut()
            .expect("infallible: cache is guaranteed to be populated")
    }

    /// Ensure the cache holds the given key; on eviction call the provided callback.
    pub fn get_or_insert_with_eviction_callback<'a, F, G>(
        &'a self,
        key: &K,
        f: F,
        on_eviction: G,
    ) -> RwOptionWriteGuard<'a, (K, V)>
    where
        F: FnOnce() -> V,
        G: FnOnce(K, V),
    {
        if !self.data.is_some_and(|(k, _)| k == key) {
            if let Some((old_key, old_value)) = self.data.take() {
                on_eviction(old_key, old_value);
            }
            self.data.set((key.clone(), f()));
        }

        self.data
            .get_mut()
            .expect("infallible: cache is guaranteed to be populated")
    }

    /// Async variant: initialize with async function if missing or different.
    pub async fn get_or_insert_with_async<'a, F>(
        &'a self,
        key: K,
        f: F,
    ) -> RwOptionWriteGuard<'a, (K, V)>
    where
        F: Future<Output = V>,
    {
        // Check with borrowed key first (short-lived)
        if !self.data.is_some_and(|(k, _)| k == &key) {
            let v = f.await;
            if let Some((_old_key, _old_value)) = self.data.take() {
                // dropped
            }
            self.data.set((key, v));
        }

        self.data
            .get_mut()
            .expect("infallible: cache is guaranteed to be populated")
    }
}
