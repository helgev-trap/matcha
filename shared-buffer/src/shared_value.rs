use std::sync::Arc;

use arc_swap::{ArcSwap, Guard};

use super::context::BufferContext;

// ---------------------------------------------------------------------------
// SignalContext (internal)
// ---------------------------------------------------------------------------

/// Determines which [`BufferContext`] a [`SharedValue`] signals on `store()`.
enum SignalContext {
    /// Looks up the global context on each `store()`.
    /// Before `init_global()` is called, signaling is a no-op.
    Global,
    /// A fixed custom context supplied at construction time.
    Custom(Arc<BufferContext>),
}

impl SignalContext {
    fn signal(&self) {
        match self {
            Self::Global => {
                if let Some(ctx) = BufferContext::try_global() {
                    ctx.signal();
                }
            }
            Self::Custom(ctx) => ctx.signal(),
        }
    }
}

// ---------------------------------------------------------------------------
// SharedValue<T>
// ---------------------------------------------------------------------------

/// A thread-safe value buffer that automatically wakes the event loop on write.
///
/// - `store()` is callable with `&self` from any thread.
/// - `store()` atomically replaces the value and sends a wakeup signal to the
///   event loop via the associated [`BufferContext`].
/// - The internal implementation (`arc-swap`) is fully hidden behind this API.
///
/// # Using the global context (common case)
///
/// ```rust
/// let v = SharedValue::new(0.0f32);
/// // Safe to call before the event loop starts; the signal is a no-op until then.
/// v.store(1.0);
/// ```
///
/// # Using a custom context
///
/// ```rust
/// let ctx = BufferContext::new();
/// let v = SharedValue::new_in(0.0f32, ctx);
/// ```
pub struct SharedValue<T: Send + Sync + 'static> {
    inner: ArcSwap<T>,
    ctx: SignalContext,
}

impl<T: Send + Sync + 'static> SharedValue<T> {
    /// Creates a `SharedValue` backed by the global context.
    ///
    /// Safe to call before [`BufferContext::init_global()`].
    pub fn new(value: T) -> Self {
        Self {
            inner: ArcSwap::from_pointee(value),
            ctx: SignalContext::Global,
        }
    }

    /// Creates a `SharedValue` backed by a custom context.
    pub fn new_in(value: T, ctx: Arc<BufferContext>) -> Self {
        Self {
            inner: ArcSwap::from_pointee(value),
            ctx: SignalContext::Custom(ctx),
        }
    }

    /// Replaces the stored value and sends a wakeup signal to the event loop.
    ///
    /// Callable with `&self` from any thread.
    pub fn store(&self, value: T) {
        self.inner.store(Arc::new(value));
        self.ctx.signal();
    }

    /// Returns the current value as a zero-copy guard.
    ///
    /// The returned `Guard` implements `Deref<Target = T>` and keeps the
    /// inner `Arc` alive for its lifetime.
    pub fn load(&self) -> Guard<Arc<T>> {
        self.inner.load()
    }

    /// Returns a clone of the current value.
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        T::clone(&self.inner.load())
    }
}
