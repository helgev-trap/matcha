use std::sync::Arc;

use crate::event::device_event::{DeviceEvent, DeviceEventState};
use crate::event::raw_device_event::{RawDeviceEvent, RawDeviceId};
use crate::event::window_event::{WindowEvent, WindowEventState};
use crate::ui_arch::window_model::WindowModel;
use crate::window::WindowId;
use crate::window_manager::WindowManager;

pub struct Application<M: WindowModel> {
    // runtime
    tokio_runtime: tokio::runtime::Runtime,

    // gpu resources
    gpu: gpu_utils::gpu::Gpu,

    // ui resources
    ui: crate::ui_arch::UiArch<M>,

    window_manager: Arc<WindowManager>,

    /// Way to send events to the event loop from outside.
    /// This is ensured to be Some after calling `Application::run()`
    event_loop_proxy: Option<Box<dyn ApplicationLoopProxy<M::Message>>>,
}

/// Construction and running
impl<M: WindowModel> Application<M> {
    pub fn new() -> Self {
        todo!()
    }

    #[cfg(feature = "winit")]
    pub fn run_on_winit(self) -> Result<Self, winit::error::EventLoopError> {
        todo!()
    }
}

/// Lifecycle events
impl<M: WindowModel> Application<M> {
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
        // Surfaces are disabled asynchronously; windows remain tracked until the model removes them.
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
impl<M: WindowModel> Application<M> {
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
impl<M: WindowModel> Application<M> {
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
                    self.ui.device_event(app_ctrl, window_id, event);
                }
            }));
    }
}

/// Raw device event
impl<M: WindowModel> Application<M> {
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
impl<M: WindowModel> Application<M> {
    pub(crate) fn event_loop_commands(&self, _cmd: ApplicationCommand) {
        todo!()
    }
}

/// User event
impl<M: WindowModel> Application<M> {
    /// Called when a `BufferUpdated` event is received from the bridge thread.
    pub(crate) fn buffer_updated(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.update(&self.window_manager, app_ctrl, &self.gpu);
    }

    pub(crate) fn backend_message(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        msg: M::Message,
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
pub(crate) fn spawn_bridge_thread<M: WindowModel>(
    proxy: winit::event_loop::EventLoopProxy<crate::winit_interface::WinitUserMessage<M>>,
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
///
/// TODO: Wrap and abstract `winit::application::ApplicationHandler::new_events`
impl<M: WindowModel> Application<M> {
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
        // currently do nothing
    }
}

/// Currently not supported
impl<M: WindowModel> Application<M> {
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
