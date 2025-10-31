use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

pub struct BackPropDirty {
    inner: Arc<BackPropDirtyInner>,
}

struct BackPropDirtyInner {
    flag: AtomicBool,

    parent: Option<Weak<BackPropDirtyInner>>,
}

impl BackPropDirtyInner {
    fn back_propagate_dirty(&self) {
        let was_dirty = self.flag.fetch_or(true, Ordering::AcqRel);
        if was_dirty {
            return;
        }

        let mut opt_parent = self.parent.as_ref().and_then(|w| w.upgrade());
        while let Some(p) = opt_parent {
            let prev = p.flag.fetch_or(true, Ordering::AcqRel);
            if prev {
                break;
            }
            opt_parent = p.parent.as_ref().and_then(|w| w.upgrade());
        }
    }
}

impl Default for BackPropDirty {
    fn default() -> Self {
        Self::new(false)
    }
}

impl BackPropDirty {
    /// Create a root dirty flag node.
    /// Initial state: clean (false). Set to true via `mark_dirty`.
    pub fn new(init: bool) -> Self {
        Self {
            inner: Arc::new(BackPropDirtyInner {
                flag: AtomicBool::new(init),
                parent: None,
            }),
        }
    }

    /// Create a child node linked to a parent.
    /// Initial state: clean (false). If construction itself should trigger upstream
    /// work, call `mark_dirty` after creation explicitly.
    pub fn with_parent(parent: &BackPropDirty) -> Self {
        Self {
            inner: Arc::new(BackPropDirtyInner {
                flag: AtomicBool::new(false),
                parent: Some(Arc::downgrade(&parent.inner)),
            }),
        }
    }

    pub fn make_child(&self) -> BackPropDirty {
        BackPropDirty::with_parent(self)
    }

    /// Mark this node dirty and propagate to ancestors (short-circuits if already dirty).
    pub fn mark_dirty(&self) {
        self.inner.back_propagate_dirty();
    }

    /// Observe dirty state (Acquire), without clearing it.
    pub fn is_dirty(&self) -> bool {
        self.inner.flag.load(Ordering::Acquire)
    }

    /// Atomically take & clear the dirty flag.
    /// Returns true if it was dirty before this call.
    /// Use pattern: while flag.take_dirty() { /* rebuild */ }
    pub fn take_dirty(&self) -> bool {
        self.inner.flag.swap(false, Ordering::AcqRel)
    }

    /// (Legacy) Clear dirty flag unconditionally. Prefer `take_dirty`.
    pub fn clear_dirty(&self) {
        self.inner.flag.store(false, Ordering::Release);
    }
}
