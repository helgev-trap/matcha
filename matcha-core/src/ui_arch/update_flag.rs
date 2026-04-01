use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct UpdateFlags {
    flag: AtomicBool,
}

impl UpdateFlags {
    pub const fn new_true() -> Self {
        Self { flag: AtomicBool::new(true) }
    }

    pub const fn new_false() -> Self {
        Self { flag: AtomicBool::new(false) }
    }

    pub fn set(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    pub fn value(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    pub fn clear(&self) {
        self.flag.store(false, Ordering::Relaxed);
    }
}
