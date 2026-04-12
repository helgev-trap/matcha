use dashmap::DashMap;
use parking_lot::Mutex;
use std::sync::Weak;
use std::{any::Any, sync::Arc};

use super::window::AnyWindowWidgetInstance;
use crate::window::{Window, WindowConfig, WindowError};
use crate::{adapter::EventLoop, window::WindowId};

// ----------------------------------------------------------------------------
// EventSender / EventReceiver
// ----------------------------------------------------------------------------

/// Type-erased sender for messages from Component background tasks back to TreeApp.
///
/// Clone-able; each clone sends to the same channel. Use `emit()` to post a
/// message that TreeApp will downcast to `C::Message` in `buffer_updated()`.
#[derive(Clone)]
pub struct EventSender {
    sender: tokio::sync::mpsc::UnboundedSender<Box<dyn Any + Send>>,
}

impl EventSender {
    pub(super) fn new(sender: tokio::sync::mpsc::UnboundedSender<Box<dyn Any + Send>>) -> Self {
        Self { sender }
    }

    pub fn emit<T: Any + Send + 'static>(&self, msg: T) {
        let _ = self.sender.send(Box::new(msg));
    }
}

/// Paired receiver for `EventSender`.
pub(super) struct EventReceiver {
    receiver: tokio::sync::mpsc::UnboundedReceiver<Box<dyn Any + Send>>,
}

impl EventReceiver {
    pub(super) fn new(receiver: tokio::sync::mpsc::UnboundedReceiver<Box<dyn Any + Send>>) -> Self {
        Self { receiver }
    }

    pub(super) fn try_recv(
        &mut self,
    ) -> Result<Box<dyn Any + Send>, tokio::sync::mpsc::error::TryRecvError> {
        self.receiver.try_recv()
    }
}

// ----------------------------------------------------------------------------
// RenderCtx
// ----------------------------------------------------------------------------

/// Context available during widget rendering.
///
/// A subset of [`UiContext`] that does not carry the event-loop reference, so
/// it can be constructed inside [`Application::render`] where no event loop is
/// present. Custom widget implementations receive this type in
/// [`Widget::render`](super::widget::Widget::render).
pub struct RenderCtx<'a> {
    pub(super) runtime_handle: &'a tokio::runtime::Handle,
    pub(super) event_sender: &'a EventSender,
    pub(super) gpu: &'a gpu_utils::gpu::Gpu,
    pub(super) window_registry: &'a DashMap<WindowId, Weak<Mutex<dyn AnyWindowWidgetInstance>>>,
}

impl RenderCtx<'_> {
    pub fn runtime_handle(&self) -> tokio::runtime::Handle {
        self.runtime_handle.clone()
    }

    pub fn event_sender(&self) -> EventSender {
        self.event_sender.clone()
    }

    pub fn emit<T: Any + Send + 'static>(&self, msg: T) {
        self.event_sender.emit(msg);
    }
}

// ----------------------------------------------------------------------------
// AppContext
// ----------------------------------------------------------------------------

/// Context passed to Component lifecycle methods (init, resumed, suspended, exiting).
pub struct AppContext<'a> {
    runtime_handle: &'a tokio::runtime::Handle,
    event_sender: &'a EventSender,
}

impl<'a> AppContext<'a> {
    pub(super) fn new(
        runtime_handle: &'a tokio::runtime::Handle,
        event_sender: &'a EventSender,
    ) -> Self {
        Self {
            runtime_handle,
            event_sender,
        }
    }

    pub fn runtime_handle(&self) -> tokio::runtime::Handle {
        self.runtime_handle.clone()
    }

    /// Returns a clone of the type-erased event sender.
    /// Use `sender.emit(your_message)` in spawned tasks to wake up TreeApp.
    pub fn event_sender(&self) -> EventSender {
        self.event_sender.clone()
    }

    /// Convenience: emit a message directly (no need to call `event_sender()` first).
    pub fn emit<T: Any + Send + 'static>(&self, msg: T) {
        self.event_sender.emit(msg);
    }
}

// ----------------------------------------------------------------------------
// UiContext
// ----------------------------------------------------------------------------

/// Context passed to Component UI methods (view, update, input).
///
/// Provides access to the GPU resources needed for widget rendering and
/// to the internal window registration. Contains a [`RenderCtx`] plus an
/// event-loop reference for window creation.
pub struct UiContext<'a> {
    pub(super) render_ctx: RenderCtx<'a>,
    pub(super) event_loop: &'a dyn EventLoop,
}

impl UiContext<'_> {
    /// Returns the render-only context embedded in this `UiContext`.
    ///
    /// Pass this to leaf widget render implementations that don't need the
    /// event loop.
    pub fn as_render_ctx(&self) -> &RenderCtx<'_> {
        &self.render_ctx
    }

    pub fn runtime_handle(&self) -> tokio::runtime::Handle {
        self.render_ctx.runtime_handle.clone()
    }

    pub fn event_sender(&self) -> EventSender {
        self.render_ctx.event_sender.clone()
    }

    pub fn emit_event(&self, event: Box<dyn std::any::Any + Send>) {
        self.render_ctx.event_sender.emit(event);
    }

    pub fn register_window_instance(&self, instance: Arc<Mutex<dyn AnyWindowWidgetInstance>>) {
        let id = instance.lock().window_id();
        self.render_ctx
            .window_registry
            .insert(id, Arc::downgrade(&instance));
    }

    pub fn create_window(&self, config: &WindowConfig) -> Result<Window, WindowError> {
        self.render_ctx.gpu.with_device_queue(|device, _queue| {
            Window::new(
                config,
                self.event_loop,
                self.render_ctx.gpu.instance(),
                device,
            )
        })
    }
}
