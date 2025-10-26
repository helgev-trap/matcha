use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

/// Utility wrapper around an `RwLock<Option<T>>`.
/// - Provides convenient guarded access to the inner value when present.
/// - Read guards are created only when the option is `Some`.
/// - While a guard (read or write) is held the inner value cannot become `None`,
///   because a write lock is required to change it.
///
/// Safety: acquiring a write lock while holding a read guard will deadlock.
/// Drop any read guards before calling methods that take a write lock (e.g. `take()`).
pub struct RwOption<T> {
    inner: RwLock<Option<T>>,
}

/// Read guard type for the inner `T` (mapped read guard).
pub type RwOptionReadGuard<'a, T> = MappedRwLockReadGuard<'a, T>;

/// Write-capable guard type for the inner `T` (mapped write guard).
pub type RwOptionWriteGuard<'a, T> = MappedRwLockWriteGuard<'a, T>;

impl<T> Default for RwOption<T> {
    fn default() -> Self {
        RwOption::new()
    }
}

impl<T> RwOption<T> {
    pub fn new() -> Self {
        RwOption {
            inner: RwLock::new(None),
        }
    }

    /// Replace the inner value with `Some(value)`.
    ///
    /// This takes a write lock and stores the provided value.
    pub fn set(&self, value: T) {
        let mut lock = self.inner.write();
        *lock = Some(value);
    }

    /// Return a read guard to the inner value if it is `Some`.
    ///
    /// The returned guard is a mapped `RwLockReadGuard<&T>` that provides a `&T`.
    /// If the option is `None`, `None` is returned.
    pub fn get(&'_ self) -> Option<RwOptionReadGuard<'_, T>> {
        let g = self.inner.read();
        if g.is_some() {
            let mapped = RwLockReadGuard::map(g, |opt| {
                // This branch only runs when `opt.is_some()` is true.
                opt.as_ref()
                    .expect("RwOption::get: value must be Some here")
            });
            Some(mapped)
        } else {
            None
        }
    }

    pub fn get_or_insert(&'_ self, value: T) -> RwOptionReadGuard<'_, T> {
        self.get_or_insert_with(|| value)
    }

    pub fn get_or_insert_default(&'_ self) -> RwOptionReadGuard<'_, T>
    where
        T: Default,
    {
        self.get_or_insert_with(T::default)
    }

    /// If the inner value is `None`, initialize it using `f` and return a read guard.
    ///
    /// Uses double-checked locking: try a read lock first, then take a write lock
    /// to initialize the value if still absent, and finally downgrade the write lock
    /// to a read guard to return `&T`.
    pub fn get_or_insert_with<F>(&'_ self, f: F) -> RwOptionReadGuard<'_, T>
    where
        F: FnOnce() -> T,
    {
        if let Some(g) = self.get() {
            return g;
        }

        // If still None, initialize under write lock.
        let mut w = self.inner.write();
        if w.is_none() {
            *w = Some(f());
        }

        // Downgrade to a read guard and map to &T.
        let r = RwLockWriteGuard::downgrade(w);

        RwLockReadGuard::map(r, |opt| {
            opt.as_ref()
                .expect("initialized above or pre-existing Some")
        })
    }

    /// Fallible version: initialize with a function that may return an error.
    ///
    /// If the initializer returns `Err`, the option is left as `None` and the error
    /// is propagated.
    pub fn get_or_try_insert_with<E, F>(&'_ self, f: F) -> Result<RwOptionReadGuard<'_, T>, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        if let Some(g) = self.get() {
            return Ok(g);
        }

        let mut w = self.inner.write();
        if w.is_none() {
            *w = Some(f()?);
        }

        let r = RwLockWriteGuard::downgrade(w);
        let mapped = RwLockReadGuard::map(r, |opt| {
            opt.as_ref()
                .expect("initialized above or pre-existing Some")
        });
        Ok(mapped)
    }

    /// Return a write-capable guard to the inner value if it is `Some`.
    ///
    /// The returned guard allows modifying the inner `T`. If the option is `None`,
    /// `None` is returned.
    pub fn get_mut(&'_ self) -> Option<RwOptionWriteGuard<'_, T>> {
        let w = self.inner.write();
        if w.is_some() {
            let mapped = RwLockWriteGuard::map(w, |opt| {
                opt.as_mut()
                    .expect("RwOption::get_mut: value must be Some here")
            });
            Some(mapped)
        } else {
            None
        }
    }

    pub fn get_mut_or_insert(&'_ self, value: T) -> RwOptionWriteGuard<'_, T> {
        self.get_mut_or_insert_with(|| value)
    }

    pub fn get_mut_or_insert_default(&'_ self) -> RwOptionWriteGuard<'_, T>
    where
        T: Default,
    {
        self.get_mut_or_insert_with(T::default)
    }

    /// If the inner value is `None`, initialize it using `f` and return a write guard.
    ///
    /// The write lock is held while initialization and the returned guard allows
    /// mutating the stored value.
    pub fn get_mut_or_insert_with<F>(&'_ self, f: F) -> RwOptionWriteGuard<'_, T>
    where
        F: FnOnce() -> T,
    {
        // Initialize or obtain under a write lock.
        let mut w = self.inner.write();
        if w.is_none() {
            *w = Some(f());
        }

        RwLockWriteGuard::map(w, |opt| {
            opt.as_mut()
                .expect("initialized above or pre-existing Some")
        })
    }

    /// Fallible version: initialize with a function that may return an error, and return a write guard.
    ///
    /// If the initializer returns `Err`, the option is left as `None` and the error is propagated.
    pub fn get_mut_or_try_insert_with<E, F>(&'_ self, f: F) -> Result<RwOptionWriteGuard<'_, T>, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        let mut w = self.inner.write();
        if w.is_none() {
            *w = Some(f()?);
        }

        let mapped = RwLockWriteGuard::map(w, |opt| {
            opt.as_mut()
                .expect("initialized above or pre-existing Some")
        });
        Ok(mapped)
    }

    /// Execute a read-only closure if the inner value is present.
    ///
    /// Returns `Some(result)` if the closure was called, otherwise `None`.
    pub fn with_read<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        self.get().map(|g| f(&*g))
    }

    /// Execute a read-only closure, initializing the value first if absent.
    ///
    /// Always returns the closure's result.
    pub fn with_read_or_insert_with<R, FI, F>(&self, init: FI, f: F) -> R
    where
        FI: FnOnce() -> T,
        F: FnOnce(&T) -> R,
    {
        let g = self.get_or_insert_with(init);
        f(&*g)
    }

    /// Execute a mutable closure if the inner value is present.
    ///
    /// Returns `Some(result)` if the closure was called, otherwise `None`.
    pub fn with_write<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        self.get_mut().map(|mut g| f(&mut *g))
    }

    /// Execute a mutable closure, initializing the value first if absent.
    ///
    /// Always returns the closure's result.
    pub fn with_write_or_insert_with<R, FI, F>(&self, init: FI, f: F) -> R
    where
        FI: FnOnce() -> T,
        F: FnOnce(&mut T) -> R,
    {
        let mut g = self.get_mut_or_insert_with(init);
        f(&mut *g)
    }

    pub fn is_some(&self) -> bool {
        self.inner.read().is_some()
    }

    pub fn is_none(&self) -> bool {
        self.inner.read().is_none()
    }

    pub fn is_some_and(&self, f: impl FnOnce(&T) -> bool) -> bool {
        self.inner.read().as_ref().is_some_and(f)
    }

    /// Take the inner value out, leaving `None`.
    ///
    /// This acquires a write lock. Do not call while holding a read guard.
    pub fn take(&self) -> Option<T> {
        let mut lock = self.inner.write();
        lock.take()
    }
}
