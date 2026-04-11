use std::sync::{Arc, Mutex};

use renderer::RenderNode;

use crate::{
    event::device_event::DeviceEvent,
    ui_arch::{
        metrics,
        ui_context::UiContext,
        widget::{View, Widget, WidgetInteractionResult, WidgetPod, WidgetUpdateError},
    },
    window::{WindowConfig, WindowId},
    window_manager::WindowHandle,
};

// ------
// Window
// ------

/// Declares a window anywhere in the view tree.
///
/// When built, creates a [`WindowWidgetInstance`] and registers it with
/// [`UiContext::register_window_instance`] so that [`UiArch`](super::UiArch) can
/// route events and rendering directly to this window.
pub struct Window {
    pub window_id: String,
    pub config: WindowConfig,
    pub view: Box<dyn View>,
}

impl View for Window {
    fn build(&self, ctx: &dyn UiContext) -> WidgetPod {
        let handle = ctx.create_window(&self.config).unwrap();
        let window_id = handle.id();
        let inner_widget = self.view.build(ctx);
        let instance = Arc::new(Mutex::new(WindowWidgetInstance::new(
            window_id,
            handle,
            inner_widget,
        )));
        ctx.register_window_instance(
            Arc::clone(&instance) as Arc<Mutex<dyn AnyWindowWidgetInstance>>
        );
        WidgetPod::new(&self.window_id, WindowWidget { instance })
    }
}

// ------------
// WindowWidget
// ------------

/// The [`Widget`] counterpart of [`Window`].
///
/// Holds the strong [`Arc`] to the [`WindowWidgetInstance`].
/// This is a zero-size widget in the parent's layout — rendering and input for
/// the window's content are handled by [`UiArch`](super::UiArch) directly via
/// the window registry, never through the parent widget tree.
pub struct WindowWidget {
    instance: Arc<Mutex<WindowWidgetInstance>>,
}

impl Widget for WindowWidget {
    type View = Window;

    fn update(&mut self, view: &Window, ctx: &dyn UiContext) -> WidgetInteractionResult {
        // The window already exists; just keep the inner widget in sync.
        // Registration was done in Window::build() and is not repeated here.
        let mut guard = self.instance.lock().unwrap();
        match guard.widget.try_update(view.view.as_ref(), ctx) {
            Ok(result) => result,
            Err(WidgetUpdateError::TypeMismatch) => {
                guard.widget = view.view.build(ctx);
                WidgetInteractionResult::LayoutNeeded
            }
        }
    }

    fn device_input(
        &mut self,
        _bounds: [f32; 2],
        _event: &DeviceEvent,
        _ctx: &dyn UiContext,
    ) -> WidgetInteractionResult {
        // Input does not cross window boundaries; handled by UiArch per-window.
        WidgetInteractionResult::NoChange
    }

    fn measure(&self, _constraints: &metrics::Constraints, _ctx: &dyn UiContext) -> [f32; 2] {
        // Zero-size: the window occupies no space in the parent layout.
        [0.0, 0.0]
    }

    fn render(&mut self, _bounds: [f32; 2], _ctx: &dyn UiContext) -> RenderNode {
        // Nothing to render in the parent tree; the window draws to its own surface.
        RenderNode::new()
    }
}

// --------------------
// WindowWidgetInstance
// --------------------

/// Live state for one OS window that lives inside the widget tree.
///
/// `window_id` is stored outside any lock because it is immutable after creation.
/// The `widget` field is accessed through the owning [`WindowWidget`]'s methods.
pub struct WindowWidgetInstance {
    window_id: WindowId,
    /// Keeps the OS window alive. Dropping this Arc (when the last strong ref goes away)
    /// triggers `WindowHandle::drop`, which removes the window from `WindowManager`.
    handle: WindowHandle,
    widget: WidgetPod,
}

impl WindowWidgetInstance {
    pub fn new(window_id: WindowId, handle: WindowHandle, widget: WidgetPod) -> Self {
        Self {
            window_id,
            handle,
            widget,
        }
    }
}

// -------------------------
// AnyWindowWidgetInstance
// -------------------------

/// Type-erased interface for [`WindowWidgetInstance`].
///
/// [`UiArch`](super::UiArch) stores `Weak<Mutex<dyn AnyWindowWidgetInstance>>` in its
/// registry keyed by [`WindowId`]. The strong [`Arc`] lives in the owning [`WindowWidget`];
/// when the window is removed from the view tree the widget is dropped, the `Arc` count
/// reaches zero, and `UiArch`'s `Weak` becomes dead (window is destroyed automatically
/// via [`WindowHandle`]'s [`Drop`] impl).
pub trait AnyWindowWidgetInstance: Send + Sync {
    fn window_id(&self) -> WindowId;
    fn size(&self) -> [f32; 2];
    fn device_input(
        &mut self,
        bounds: [f32; 2],
        event: &DeviceEvent,
        ctx: &dyn UiContext,
    ) -> WidgetInteractionResult;
    fn render(&mut self, bounds: [f32; 2], ctx: &dyn UiContext) -> RenderNode;
    fn measure(&self, constraints: &metrics::Constraints, ctx: &dyn UiContext) -> [f32; 2];
}

impl AnyWindowWidgetInstance for WindowWidgetInstance {
    fn window_id(&self) -> WindowId {
        self.window_id
    }

    fn size(&self) -> [f32; 2] {
        self.handle.size()
    }

    fn device_input(
        &mut self,
        bounds: [f32; 2],
        event: &DeviceEvent,
        ctx: &dyn UiContext,
    ) -> WidgetInteractionResult {
        self.widget.device_input(bounds, event, ctx)
    }

    fn render(&mut self, bounds: [f32; 2], ctx: &dyn UiContext) -> RenderNode {
        self.widget.render(bounds, ctx)
    }

    fn measure(&self, constraints: &metrics::Constraints, ctx: &dyn UiContext) -> [f32; 2] {
        self.widget.measure(constraints, ctx)
    }
}
