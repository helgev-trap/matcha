use std::sync::Arc;

use crate::event::device_event::DeviceEvent;
use crate::event::raw_device_event::{RawDeviceEvent, RawDeviceId};
use crate::event::window_event::WindowEvent;
use crate::event_sender::{EventReceiver, EventSender};
use crate::ui_arch::component::Component;
use crate::window::WindowId;
use crate::window_manager::WindowManager;

pub struct Application<C: Component> {
    // runtime
    tokio_runtime: tokio::runtime::Runtime,

    // gpu resources
    gpu: gpu_utils::gpu::Gpu,

    // ui resources
    ui: crate::ui_arch::UiArch<C>,

    window_manager: Arc<WindowManager>,

    /// Sender handle shared with every `UiContext` instance.
    /// The matching `EventReceiver` is returned from `Application::new()`.
    event_sender: EventSender,

    /// Way to send events to the event loop from outside.
    /// This is ensured to be Some after calling `Application::run()`
    event_loop_proxy: Option<Box<dyn ApplicationLoopProxy<C::Message>>>,
}

/// Construction and running
impl<C: Component> Application<C> {
    /// Creates a new `Application` and returns the paired `EventReceiver`.
    ///
    /// Events emitted by widgets via `ctx.emit_event()` or `ctx.event_sender().emit()`
    /// are received from `EventReceiver`.
    pub fn new(ui: C) -> (Self, EventReceiver) {
        todo!()
    }

    #[cfg(feature = "winit")]
    pub fn run_on_winit(self) -> Result<Self, winit::error::EventLoopError> {
        todo!()
    }
}

/// Lifecycle events
impl<C: Component> Application<C> {
    pub(crate) fn init(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.init(&self.tokio_runtime.handle(), app_ctrl);
    }

    pub(crate) fn resumed(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.resumed(&self.tokio_runtime.handle(), app_ctrl);
    }

    pub(crate) fn create_window(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.update(
            &self.tokio_runtime.handle(),
            app_ctrl,
            &self.window_manager,
            &self.gpu,
        );
    }

    pub(crate) fn destroy_window(&self, _app_ctrl: &impl ApplicationControler) {
        let _ = self.window_manager.disable_all_windows();
    }

    pub(crate) fn suspended(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.suspended(&self.tokio_runtime.handle(), app_ctrl);
    }

    pub(crate) fn exiting(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.exiting(&self.tokio_runtime.handle(), app_ctrl);
    }
}

/// Window events
impl<C: Component> Application<C> {
    pub(crate) fn window_event(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // TODO: reconsider this async / sync boundary
        let processed =
            self.tokio_runtime
                .block_on(self.window_manager.with_window(window_id, |window| {
                    window.window_event_state.process_event(&event)
                }));

        if let Some(Some(event)) = processed {
            self.ui.window_event(app_ctrl, window_id, event);
        }
    }
}

/// Device event
impl<C: Component> Application<C> {
    pub(crate) fn device_event(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        window_id: WindowId,
        event: DeviceEvent,
    ) {
        // TODO: reconsider this async / sync boundary
        let processed =
            self.tokio_runtime
                .block_on(self.window_manager.with_window(window_id, |window| {
                    window.device_event_state.process_event(&event)
                }));

        if let Some(Some(event)) = processed {
            self.ui.device_event(
                window_id,
                event,
                &self.tokio_runtime.handle(),
                app_ctrl,
                &self.window_manager,
                &self.gpu,
            );
        }
    }
}

/// Raw device event
impl<C: Component> Application<C> {
    pub(crate) fn raw_device_event(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        raw_device_id: RawDeviceId,
        raw_event: RawDeviceEvent,
    ) {
        self.ui.raw_device_event(app_ctrl, raw_device_id, raw_event);
    }
}

/// Event Loop Commands
impl<C: Component> Application<C> {
    pub(crate) fn event_loop_commands(&self, _cmd: ApplicationCommand) {
        todo!()
    }
}

/// User event
impl<C: Component> Application<C> {
    /// Called when a `BufferUpdated` event is received from the bridge thread.
    pub(crate) fn buffer_updated(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.update(
            &self.tokio_runtime.handle(),
            app_ctrl,
            &self.window_manager,
            &self.gpu,
        );
    }

    pub(crate) fn backend_message(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        msg: C::Message,
    ) {
        self.ui.user_event(
            msg,
            &self.tokio_runtime.handle(),
            app_ctrl,
            &self.window_manager,
            &self.gpu,
        );
    }
}

/// Spawns the buffer bridge thread.
///
/// The bridge thread blocks on [`BufferContext::wait_for_signal()`] and forwards
/// each signal to the winit event loop via `EventLoopProxy::send_event()`,
/// waking it from `ControlFlow::Wait`.
///
/// Call once after the winit `EventLoopProxy` has been obtained.
#[cfg(feature = "winit")]
pub(crate) fn spawn_bridge_thread<C: Component>(
    proxy: winit::event_loop::EventLoopProxy<crate::winit_interface::WinitUserMessage<C>>,
) -> std::thread::JoinHandle<()> {
    shared_buffer::BufferContext::init_global();
    let ctx = shared_buffer::BufferContext::global().clone();

    std::thread::Builder::new()
        .name("matcha-buffer-bridge".into())
        .spawn(move || {
            while ctx.wait_for_signal() {
                proxy
                    .send_event(crate::winit_interface::WinitUserMessage::BufferUpdated)
                    .ok();
            }
        })
        .expect("failed to spawn buffer bridge thread")
}

/// Polling event
impl<C: Component> Application<C> {
    pub(crate) fn poll(&mut self, _app_ctrl: &impl ApplicationControler) {
        todo!()
    }

    pub(crate) fn resume_time_reached(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        _start: std::time::Instant,
        _requested_resume: std::time::Instant,
    ) {
        self.ui.update(
            &self.tokio_runtime.handle(),
            app_ctrl,
            &self.window_manager,
            &self.gpu,
        );
    }

    pub(crate) fn wait_cancelled(
        &mut self,
        _app_ctrl: &impl ApplicationControler,
        _start: std::time::Instant,
        _requested_resume: Option<std::time::Instant>,
    ) {
    }
}

/// Currently not supported
impl<C: Component> Application<C> {
    pub(crate) fn about_to_wait(&mut self, _app_ctrl: &impl ApplicationControler) {}

    pub(crate) fn memory_warning(&mut self, _app_ctrl: &impl ApplicationControler) {}
}

// -------------------
// API type definition
// -------------------

pub(crate) trait ApplicationControler: crate::window::WindowControler {}

pub(crate) enum ApplicationCommand {
    Exit,
}

pub(crate) trait ApplicationLoopProxy<BackendMessage: Send + 'static> {}
