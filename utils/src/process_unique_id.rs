use parking_lot::Mutex;
use std::fmt;

/// A unique identifier for a process, guaranteed to be unique within the current process execution.
///
/// This implementation uses a `parking_lot::Mutex` to protect a global `u128` counter.
/// While atomic types would be ideal, stable Rust does not yet provide `AtomicU128`.
/// `parking_lot::Mutex` is a high-performance mutex that is suitable for this purpose.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessUniqueId(u128);

// A global counter protected by a mutex to ensure uniqueness.
// Starts at 1, leaving 0 as a potential sentinel value.
static COUNTER: Mutex<u128> = Mutex::new(0);

impl ProcessUniqueId {
    /// Gets a new, unique identifier.
    ///
    /// # Panics
    ///
    /// Panics if the u128 counter overflows `u128::MAX`. This is practically impossible.
    pub fn get() -> Self {
        let mut counter = COUNTER.lock();
        *counter += 1;
        if *counter == u128::MAX {
            // This should never happen in realistic scenarios.
            // It would take astronomical time to overflow even with intense usage.
            panic!("ProcessUniqueId counter has overflowed!");
        }
        ProcessUniqueId(*counter)
    }
}

impl fmt::Debug for ProcessUniqueId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ProcessUniqueId").field(&self.0).finish()
    }
}
