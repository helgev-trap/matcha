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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    // -------- Cache (single-threaded) --------

    #[test]
    fn cache_basic_set_get_clear() {
        let mut c: Cache<String, i32> = Cache::new();
        assert!(c.get().is_none());
        assert!(c.get_mut().is_none());

        c.set("a".to_string(), 1);

        {
            let (k, v) = c.get().expect("value must be present");
            assert_eq!(k, "a");
            assert_eq!(*v, 1);
        }

        {
            let (k, v) = c.get_mut().expect("value must be present");
            assert_eq!(k, "a");
            *v += 1;
        }

        {
            let (k, v) = c.get().expect("value must be present");
            assert_eq!(k, "a");
            assert_eq!(*v, 2);
        }

        c.clear();
        assert!(c.get().is_none());
        assert!(c.get_mut().is_none());
    }

    #[test]
    fn cache_get_or_insert_variants_and_closure_calls() {
        let mut c: Cache<String, i32> = Cache::new();
        let calls = Arc::new(AtomicUsize::new(0));

        // insert with closure
        {
            let calls1 = calls.clone();
            let (k, v) = c.get_or_insert_with(&"a".to_string(), || {
                calls1.fetch_add(1, Ordering::SeqCst);
                5
            });
            assert_eq!(k, "a");
            assert_eq!(*v, 5);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        // same key -> closure should NOT be called, value unchanged
        {
            let calls2 = calls.clone();
            let (k, v) = c.get_or_insert_with(&"a".to_string(), || {
                calls2.fetch_add(1, Ordering::SeqCst);
                999
            });
            assert_eq!(k, "a");
            assert_eq!(*v, 5);
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "closure must not run for same key"
        );

        // different key via get_or_insert -> value replaced
        {
            let (k, v) = c.get_or_insert(&"b".to_string(), 7);
            assert_eq!(k, "b");
            assert_eq!(*v, 7);
        }

        // same key with default -> no replacement
        {
            let (k, v) = c.get_or_insert_default(&"b".to_string());
            assert_eq!(k, "b");
            assert_eq!(*v, 7);
        }

        // new key with default -> insert default
        {
            let (k, v) = c.get_or_insert_default(&"c".to_string());
            assert_eq!(k, "c");
            assert_eq!(*v, 0);
        }
    }

    #[test]
    fn cache_eviction_callback() {
        let mut c: Cache<String, i32> = Cache::new();
        c.set("x".to_string(), 1);

        let evicted: Arc<Mutex<Vec<(String, i32)>>> = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(AtomicUsize::new(0));

        // same key -> no eviction, no initializer call
        {
            let calls1 = calls.clone();
            let evicted1 = evicted.clone();
            let (k, v) = c.get_or_insert_with_eviction_callback(
                &"x".to_string(),
                || {
                    calls1.fetch_add(1, Ordering::SeqCst);
                    2
                },
                move |old_k, old_v| {
                    evicted1.lock().expect("poison").push((old_k, old_v));
                },
            );
            assert_eq!(k, "x");
            assert_eq!(*v, 1);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(evicted.lock().expect("poison").is_empty());

        // different key -> eviction callback must run, initializer runs once
        {
            let calls2 = calls.clone();
            let evicted2 = evicted.clone();
            let (k, v) = c.get_or_insert_with_eviction_callback(
                &"y".to_string(),
                || {
                    calls2.fetch_add(1, Ordering::SeqCst);
                    3
                },
                move |old_k, old_v| {
                    evicted2.lock().expect("poison").push((old_k, old_v));
                },
            );
            assert_eq!(k, "y");
            assert_eq!(*v, 3);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let ev = evicted.lock().expect("poison").clone();
        assert_eq!(ev, vec![("x".to_string(), 1)]);
    }

    #[test]
    fn cache_async_insert() {
        let mut c: Cache<String, i32> = Cache::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

        rt.block_on(async {
            // first insert
            {
                let calls1 = calls.clone();
                let (k, v) = c
                    .get_or_insert_with_async("k1".to_string(), async {
                        calls1.fetch_add(1, Ordering::SeqCst);
                        10
                    })
                    .await;
                assert_eq!(k.as_str(), "k1");
                assert_eq!(*v, 10);
            }

            // same key -> future must not run, value unchanged
            {
                let calls2 = calls.clone();
                let (_k, v) = c
                    .get_or_insert_with_async("k1".to_string(), async {
                        calls2.fetch_add(1, Ordering::SeqCst);
                        999
                    })
                    .await;
                assert_eq!(*v, 10);
            }
        });

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "async initializer should run only once for same key"
        );
    }

    // -------- RwCache (thread-safe) --------

    #[test]
    fn rwcache_basic_set_get_clear_and_mutate() {
        let c: RwCache<String, i32> = RwCache::new();
        assert!(c.get().is_none());
        assert!(c.get_mut().is_none());

        c.set("a".to_string(), 1);

        {
            let g = c.get().expect("present");
            assert_eq!(g.0, "a");
            assert_eq!(g.1, 1);
        }

        {
            let mut g = c.get_mut().expect("present");
            g.1 += 2;
        }

        {
            let g = c.get().expect("present");
            assert_eq!(g.0, "a");
            assert_eq!(g.1, 3);
        }

        // drop guards before clear to avoid deadlock (documented in RwOption)
        c.clear();
        assert!(c.get().is_none());
        assert!(c.get_mut().is_none());
    }

    #[test]
    fn rwcache_get_or_insert_variants_and_eviction_free_path() {
        let c: RwCache<String, i32> = RwCache::new();
        let calls = Arc::new(AtomicUsize::new(0));

        // insert with closure
        {
            let calls1 = calls.clone();
            let g = c.get_or_insert_with(&"k".to_string(), || {
                calls1.fetch_add(1, Ordering::SeqCst);
                7
            });
            assert_eq!(g.0, "k");
            assert_eq!(g.1, 7);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        // same key -> closure not called
        {
            let calls2 = calls.clone();
            let g = c.get_or_insert_with(&"k".to_string(), || {
                calls2.fetch_add(1, Ordering::SeqCst);
                999
            });
            assert_eq!(g.0, "k");
            assert_eq!(g.1, 7);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        // different key -> replacement via get_or_insert (no callback variant)
        {
            let g = c.get_or_insert(&"k2".to_string(), 3);
            assert_eq!(g.0, "k2");
            assert_eq!(g.1, 3);
        }

        // default same key -> no replacement
        {
            let g = c.get_or_insert_default(&"k2".to_string());
            assert_eq!(g.0, "k2");
            assert_eq!(g.1, 3);
        }

        // default with new key -> default inserted
        {
            let g = c.get_or_insert_default(&"k3".to_string());
            assert_eq!(g.0, "k3");
            assert_eq!(g.1, 0);
        }
    }

    #[test]
    fn rwcache_eviction_callback() {
        let c: RwCache<String, i32> = RwCache::new();
        c.set("x".to_string(), 1);

        let evicted: Arc<Mutex<Vec<(String, i32)>>> = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(AtomicUsize::new(0));

        // same key -> no eviction, no initializer
        {
            let calls1 = calls.clone();
            let evicted1 = evicted.clone();
            let g = c.get_or_insert_with_eviction_callback(
                &"x".to_string(),
                || {
                    calls1.fetch_add(1, Ordering::SeqCst);
                    2
                },
                move |old_k, old_v| {
                    evicted1.lock().expect("poison").push((old_k, old_v));
                },
            );
            assert_eq!(g.0, "x");
            assert_eq!(g.1, 1);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(evicted.lock().expect("poison").is_empty());

        // different key -> eviction happens once
        {
            let calls2 = calls.clone();
            let evicted2 = evicted.clone();
            let g = c.get_or_insert_with_eviction_callback(
                &"y".to_string(),
                || {
                    calls2.fetch_add(1, Ordering::SeqCst);
                    3
                },
                move |old_k, old_v| {
                    evicted2.lock().expect("poison").push((old_k, old_v));
                },
            );
            assert_eq!(g.0, "y");
            assert_eq!(g.1, 3);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let ev = evicted.lock().expect("poison").clone();
        assert_eq!(ev, vec![("x".to_string(), 1)]);
    }

    #[test]
    fn rwcache_async_insert() {
        let c: RwCache<String, i32> = RwCache::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

        rt.block_on(async {
            // first insert
            {
                let calls1 = calls.clone();
                let mut g = c
                    .get_or_insert_with_async("k1".to_string(), async {
                        calls1.fetch_add(1, Ordering::SeqCst);
                        11
                    })
                    .await;
                assert_eq!(g.0, "k1");
                assert_eq!(g.1, 11);
                g.1 += 1; // mutate via write guard
            }

            // same key -> future must not run, value persists (12)
            {
                let calls2 = calls.clone();
                let g = c
                    .get_or_insert_with_async("k1".to_string(), async {
                        calls2.fetch_add(1, Ordering::SeqCst);
                        999
                    })
                    .await;
                assert_eq!(g.0, "k1");
                assert_eq!(g.1, 12);
            }
        });

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "async initializer should run only once for same key"
        );
    }
}
