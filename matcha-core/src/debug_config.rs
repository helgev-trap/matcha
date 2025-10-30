use std::sync::atomic::{AtomicBool, Ordering};

/// Runtime debug configuration used to selectively disable caches for profiling.
/// All fields are AtomicBool to allow low-cost runtime toggling.
#[derive(Clone)]
pub(crate) struct DebugConfig {
    always_rebuild_widget: AtomicBool,
    disable_layout_measure_cache: AtomicBool,
    disable_layout_arrange_cache: AtomicBool,
    disable_render_node_cache: AtomicBool,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self::new(false, false, false, false)
    }
}

impl DebugConfig {
    pub(crate) fn new(
        always_rebuild_widget: bool,
        disable_layout_measure_cache: bool,
        disable_layout_arrange_cache: bool,
        disable_render_node_cache: bool,
    ) -> Self {
        Self {
            always_rebuild_widget: AtomicBool::new(always_rebuild_widget),
            disable_layout_measure_cache: AtomicBool::new(disable_layout_measure_cache),
            disable_layout_arrange_cache: AtomicBool::new(disable_layout_arrange_cache),
            disable_render_node_cache: AtomicBool::new(disable_render_node_cache),
        }
    }

    pub fn always_rebuild_widget(&self) -> bool {
        self.always_rebuild_widget.load(Ordering::Relaxed)
    }

    pub(crate) fn set_always_rebuild_widget(&self, value: bool) {
        self.always_rebuild_widget.store(value, Ordering::Relaxed);
    }

    pub fn disable_layout_measure_cache(&self) -> bool {
        self.disable_layout_measure_cache.load(Ordering::Relaxed)
    }

    pub(crate) fn set_disable_layout_measure_cache(&self, value: bool) {
        self.disable_layout_measure_cache
            .store(value, Ordering::Relaxed);
    }

    pub fn disable_layout_arrange_cache(&self) -> bool {
        self.disable_layout_arrange_cache.load(Ordering::Relaxed)
    }

    pub(crate) fn set_disable_layout_arrange_cache(&self, value: bool) {
        self.disable_layout_arrange_cache
            .store(value, Ordering::Relaxed);
    }

    pub fn disable_render_node_cache(&self) -> bool {
        self.disable_render_node_cache.load(Ordering::Relaxed)
    }

    pub(crate) fn set_disable_render_node_cache(&self, value: bool) {
        self.disable_render_node_cache
            .store(value, Ordering::Relaxed);
    }
}
