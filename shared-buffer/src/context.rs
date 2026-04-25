use std::sync::{Arc, LazyLock};

// ---------------------------------------------------------------------------
// BufferContext
// ---------------------------------------------------------------------------

/// Shared notification context for all [`SharedValue`](super::SharedValue) instances.
///
/// All buffers created from the same context share a single notifier, so any
/// write to any buffer triggers the same event-loop wakeup signal.
///
/// # Global context
///
/// The global instance is lazily initialized on first access via [`BufferContext::global()`].
/// No explicit initialization call is required.
///
/// # Custom context
///
/// To isolate a group of buffers into a separate notification channel, create
/// a custom instance with [`BufferContext::new()`] and pass it to
/// [`SharedValue::new_in()`](super::SharedValue::new_in).
pub struct BufferContext {
    notify: tokio::sync::Notify,
}

static GLOBAL: LazyLock<Arc<BufferContext>> = LazyLock::new(BufferContext::new);

impl BufferContext {
    /// Creates a new isolated context.
    ///
    /// For the common case, prefer [`global()`](Self::global).
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            notify: tokio::sync::Notify::new(),
        })
    }

    /// Returns a reference to the global context.
    ///
    /// Initialized automatically on first access.
    pub fn global() -> &'static Arc<BufferContext> {
        &GLOBAL
    }
}

impl BufferContext {
    /// Sends a wakeup signal. Non-blocking, callable from any thread.
    ///
    /// If a signal is already pending (no one has called `notified()` yet),
    /// it is coalesced — at most one wakeup is queued at a time.
    pub(crate) fn signal(&self) {
        self.notify.notify_one();
    }

    /// Waits asynchronously until a signal is received.
    ///
    /// The bridge task calls this in a `select!` loop to forward signals to
    /// the event loop.
    pub async fn notified(&self) {
        self.notify.notified().await
    }
}
