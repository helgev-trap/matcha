use std::sync::Arc;

use crate::backend::Backend;
use crate::event::device_event::{DeviceEvent, DeviceEventState};
use crate::event::raw_device_event::{RawDeviceEvent, RawDeviceId};
use crate::event::window_event::{WindowEvent, WindowEventState};
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

    /// Backend that receives outward events from the widget tree.
    backend: Arc<dyn Backend<C::Event> + Send + Sync>,

    /// Way to send events to the event loop from outside.
    /// This is ensured to be Some after calling `Application::run()`
    event_loop_proxy: Option<Box<dyn ApplicationLoopProxy<C::Message>>>,
}

/// Construction and running
impl<C: Component> Application<C> {
    pub fn new(
        model: C,
        backend: Arc<dyn Backend<C::Event> + Send + Sync>,
    ) -> Self {
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
        self.ui.init(app_ctrl);
    }

    pub(crate) fn resumed(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.resumed(app_ctrl);
    }

    pub(crate) fn create_window(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.update(&self.window_manager, app_ctrl, &self.gpu);
    }

    pub(crate) fn destroy_window(&self, _app_ctrl: &impl ApplicationControler) {
        let _ = self.window_manager.disable_all_windows();
    }

    pub(crate) fn suspended(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.suspended(app_ctrl);
    }

    pub(crate) fn exiting(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.exiting(app_ctrl);
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
        self.tokio_runtime
            .block_on(self.window_manager.with_window(window_id, |window| {
                let event = window.window_event_state.process_event(&event);

                if let Some(event) = event {
                    self.ui.window_event(app_ctrl, window_id, event);
                }
            }));
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
        self.tokio_runtime
            .block_on(self.window_manager.with_window(window_id, |window| {
                let event = window.device_event_state.process_event(&event);

                if let Some(event) = event {
                    if let Some(out_event) = self.ui.device_event(app_ctrl, window_id, event) {
                        let backend = self.backend.clone();
                        tokio::spawn(async move {
                            backend.send_event(out_event).await;
                        });
                    }
                }
            }));
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
        self.ui.update(&self.window_manager, app_ctrl, &self.gpu);
    }

    pub(crate) fn backend_message(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        msg: C::Message,
    ) {
        self.ui.user_event(app_ctrl, msg);
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
    proxy: winit::event_loop::EventLoopProxy<
        crate::winit_interface::WinitUserMessage<C>,
    >,
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
        self.ui.update(&self.window_manager, app_ctrl, &self.gpu);
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
