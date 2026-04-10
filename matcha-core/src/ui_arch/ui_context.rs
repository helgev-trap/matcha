use std::sync::{Arc, Mutex};

use super::window::AnyWindowWidgetInstance;

pub(crate) trait UiContextPubCrate {}

/// Context passed to `Widget` and `View` methods.
///
/// Will expose layout utilities, font systems, etc. in future.
/// Concrete implementations are provided by each platform integration
/// (e.g. `winit_interface`, `baseview_interface`).
pub trait UiContext: UiContextPubCrate {
    /// Registers a window widget instance with the owning [`UiArch`](super::UiArch).
    ///
    /// Called by [`WindowWidget`](super::window::WindowWidget) on every update cycle
    /// so that `UiArch` can keep its window registry up to date.
    /// `UiArch` stores only a `Weak` reference; the strong `Arc` lives in `WindowWidget`.
    fn register_window_instance(&self, instance: Arc<Mutex<dyn AnyWindowWidgetInstance>>);

    /// Returns the tokio runtime handle for spawning background tasks.
    fn runtime_handle(&self) -> tokio::runtime::Handle;
}
