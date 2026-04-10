use std::sync::{Arc, Mutex};

use renderer::RenderNode;

use crate::{
    event::device_event::DeviceEvent,
    ui_arch::{
        metrics,
        ui_context::UiContext,
        widget::{View, Widget, WidgetInteractionResult, WidgetPod},
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
pub struct Window<T> {
    pub window_id: String,
    pub config: WindowConfig,
    pub view: Box<dyn View<T>>,
}

impl<T: 'static> View<T> for Window<T> {
    fn build(&self, ctx: &dyn UiContext) -> WidgetPod<T> {
        todo!()
    }
}

// --------------------
// WindowWidgetInstance
// --------------------

/// Live state for one OS window that lives inside the widget tree.
///
/// `window_id` is stored outside any lock because it is immutable after creation.
/// The `widget` field is accessed through the owning [`WindowWidget`]'s methods.
pub struct WindowWidgetInstance<T: 'static> {
    window_id: WindowId,
    handle: WindowHandle,
    widget: WidgetPod<T>,
}

impl<T: 'static> WindowWidgetInstance<T> {
    pub fn new(window_id: WindowId, handle: WindowHandle, widget: WidgetPod<T>) -> Self {
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

/// Type-erased interface for [`WindowWidgetInstance<T>`].
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

impl<T: Send + Sync + 'static> AnyWindowWidgetInstance for WindowWidgetInstance<T> {
    fn window_id(&self) -> WindowId {
        self.window_id
    }

    fn size(&self) -> [f32; 2] {
        todo!()
    }

    fn device_input(
        &mut self,
        _bounds: [f32; 2],
        _event: &DeviceEvent,
        _ctx: &dyn UiContext,
    ) -> WidgetInteractionResult {
        todo!()
    }

    fn render(&mut self, _bounds: [f32; 2], _ctx: &dyn UiContext) -> RenderNode {
        todo!()
    }

    fn measure(&self, _constraints: &metrics::Constraints, _ctx: &dyn UiContext) -> [f32; 2] {
        todo!()
    }
}

// ------------
// WindowWidget
// ------------

/// The [`Widget`] counterpart of [`Window`].
///
/// Holds the strong [`Arc`] to the [`WindowWidgetInstance`].
/// On every [`Widget::update`] call it re-registers the instance with
/// [`UiContext::register_window_instance`] so that [`UiArch`](super::UiArch) keeps
/// its registry up to date each render cycle.
pub struct WindowWidget<T: 'static> {
    instance: Arc<Mutex<WindowWidgetInstance<T>>>,
}

impl<T: Send + Sync + 'static> Widget<T> for WindowWidget<T> {
    type View = Window<T>;

    fn update(&mut self, _view: &Window<T>, ctx: &dyn UiContext) -> WidgetInteractionResult {
        todo!()
    }

    fn device_input(
        &mut self,
        _bounds: [f32; 2],
        _event: &DeviceEvent,
        _ctx: &dyn UiContext,
    ) -> (Option<T>, WidgetInteractionResult) {
        todo!()
    }

    fn measure(&self, _constraints: &metrics::Constraints, _ctx: &dyn UiContext) -> [f32; 2] {
        todo!()
    }

    fn render(&mut self, _bounds: [f32; 2], _ctx: &dyn UiContext) -> RenderNode {
        todo!()
    }
}
