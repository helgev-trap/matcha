use std::sync::{Arc, Mutex};

use crate::event_sender::EventSender;
use crate::window::{WindowConfig, WindowError};
use crate::window_manager::WindowHandle;

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
    /// Called by [`Window`](super::window::Window) in its [`View::build`](super::widget::View::build)
    /// impl. `UiArch` stores only a `Weak` reference; the strong `Arc` lives in the
    /// [`WindowWidget`](super::window::WindowWidget) inside the tree.
    fn register_window_instance(&self, instance: Arc<Mutex<dyn AnyWindowWidgetInstance>>);

    /// Returns the tokio runtime handle for spawning background tasks.
    fn runtime_handle(&self) -> tokio::runtime::Handle;

    /// Creates a native OS window and returns a handle to it.
    ///
    /// The handle keeps the OS window alive for as long as it is held.
    /// Dropping the handle destroys the window.
    fn create_window(&self, config: &WindowConfig) -> Result<WindowHandle, WindowError>;

    /// Returns a `'static + Clone + Send + Sync` event sender handle.
    ///
    /// Clone this and move it into async tasks spawned via `ctx.runtime_handle().spawn(...)`.
    ///
    /// ```ignore
    /// fn setup(&self, ctx: &dyn UiContext) {
    ///     let sender = ctx.event_sender();
    ///     ctx.runtime_handle().spawn(async move {
    ///         sender.emit(Box::new(MyEvent::Done));
    ///     });
    /// }
    /// ```
    fn event_sender(&self) -> EventSender;

    /// Emits a type-erased event to the application event channel.
    ///
    /// Equivalent to `self.event_sender().emit(event)`.
    /// Prefer [`event_sender`](UiContext::event_sender) when emitting from async tasks.
    fn emit_event(&self, event: Box<dyn std::any::Any + Send>) {
        self.event_sender().emit(event);
    }
}
