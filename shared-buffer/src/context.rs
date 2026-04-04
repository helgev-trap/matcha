use std::sync::{mpsc, Arc, OnceLock};

use parking_lot::Mutex;

// ---------------------------------------------------------------------------
// BufferContext
// ---------------------------------------------------------------------------

/// Shared notification context for all [`SharedValue`](super::SharedValue) instances.
///
/// All buffers created from the same context share a single channel, so any
/// write to any buffer triggers the same event-loop wakeup signal.
///
/// Holds both ends of the channel, keeping signal sending and waiting in one place.
///
/// # Global context
///
/// The typical usage is the global instance obtained via [`BufferContext::global()`],
/// initialized once by calling [`BufferContext::init_global()`].
///
/// # Custom context
///
/// To isolate a group of buffers into a separate notification channel, create
/// a custom instance with [`BufferContext::new()`] and pass it to
/// [`SharedValue::new_in()`](super::SharedValue::new_in).
pub struct BufferContext {
    tx: mpsc::SyncSender<()>,
    /// `Receiver` is not `Sync`, so it is wrapped in a `Mutex`.
    /// Only the bridge thread calls `wait_for_signal()`, so lock contention never occurs.
    rx: Mutex<mpsc::Receiver<()>>,
}

static GLOBAL: OnceLock<Arc<BufferContext>> = OnceLock::new();

impl BufferContext {
    /// Creates a new context backed by a capacity-1 bounded channel.
    ///
    /// Use this when you need an isolated notification group.
    /// For the common case, prefer [`init_global()`](Self::init_global).
    pub fn new() -> Arc<Self> {
        let (tx, rx) = mpsc::sync_channel::<()>(1);
        Arc::new(Self {
            tx,
            rx: Mutex::new(rx),
        })
    }

    /// Initializes the global context.
    ///
    /// Call once after the winit `EventLoopProxy` has been obtained.
    /// Subsequent calls are silently ignored.
    pub fn init_global() {
        GLOBAL.set(Self::new()).ok();
    }

    /// Returns a reference to the global context.
    ///
    /// Panics if [`init_global()`](Self::init_global) has not been called yet.
    /// Use [`SharedValue::new()`](super::SharedValue::new) to safely create buffers
    /// before the event loop starts; `store()` is a no-op for signals until then.
    pub fn global() -> &'static Arc<BufferContext> {
        GLOBAL.get().expect(
            "BufferContext is not initialized. \
             Call BufferContext::init_global() before using SharedValue::store().",
        )
    }

    /// Returns the global context, or `None` if not yet initialized.
    ///
    /// Used internally by [`SharedValue`](super::SharedValue).
    pub(crate) fn try_global() -> Option<&'static Arc<BufferContext>> {
        GLOBAL.get()
    }
}

impl BufferContext {
    /// Sends a wakeup signal. Non-blocking.
    ///
    /// If the channel is already full (a signal is pending), the send is dropped.
    /// This coalesces multiple rapid writes into a single wakeup.
    pub(crate) fn signal(&self) {
        self.tx.try_send(()).ok();
    }

    /// Blocks until a signal is received.
    ///
    /// The bridge thread calls this in a loop to forward signals to the event loop.
    /// Returns `false` when all senders have been dropped, allowing the loop to exit.
    pub fn wait_for_signal(&self) -> bool {
        self.rx.lock().recv().is_ok()
    }
}
