use std::collections::HashMap;

use crate::event::device_event::{DeviceEvent, DeviceEventState};
use crate::event::raw_device_event::{RawDeviceEvent, RawDeviceId};
use crate::event::window_event::{WindowEvent, WindowEventState};
use crate::window::{WindowId, WindowManager};

pub struct Application<BackendMessage: Send + 'static> {
    // runtime
    tokio_runtime: tokio::runtime::Runtime,

    // gpu resources
    gpu: gpu_utils::gpu::Gpu,

    // ui resources
    ui: crate::ui_arch::UiArch<BackendMessage>,

    window_manager: WindowManager,
    device_event_state: HashMap<WindowId, DeviceEventState, fxhash::FxBuildHasher>,
    window_event_state: HashMap<WindowId, WindowEventState, fxhash::FxBuildHasher>,

    // winit interface
    /// winit event loop proxy
    /// This is ensured to be Some after calling `Application::run()`
    event_loop_proxy: Option<Box<dyn ApplicationLoopProxy<BackendMessage>>>,
}

/// Construction and running
impl<BackendMessage: Send + 'static> Application<BackendMessage> {
    pub fn new() -> Self {
        todo!()
    }

    pub fn run_on_winit(self) -> Result<Self, winit::error::EventLoopError> {
        todo!()
    }
}

/// Lifecycle events
impl<BackendMessage: Send + 'static> Application<BackendMessage> {
    pub(crate) fn init(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.init(app_ctrl);
    }

    pub(crate) fn resumed(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.resumed(app_ctrl);
    }

    pub(crate) fn create_window(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.update(&self.window_manager, app_ctrl);
    }

    pub(crate) fn destroy_window(&self, app_ctrl: &impl ApplicationControler) {
        self.window_manager.disable_all_windows();
    }

    pub(crate) fn suspended(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.suspended(app_ctrl);
    }

    pub(crate) fn exiting(&mut self, app_ctrl: &impl ApplicationControler) {
        self.ui.exiting(app_ctrl);
    }
}

/// Window events
impl<BackendMessage: Send + 'static> Application<BackendMessage> {
    pub(crate) fn window_event(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = self
            .window_event_state
            .entry(window_id)
            .or_insert_with(|| WindowEventState::new());
        let event = state.process_event(&event);

        if let Some(event) = event {
            self.ui.window_event(app_ctrl, window_id, event);
        }
    }
}

/// Device event
impl<BackendMessage: Send + 'static> Application<BackendMessage> {
    pub(crate) fn device_event(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        window_id: WindowId,
        event: DeviceEvent,
    ) {
        let state = self
            .device_event_state
            .entry(window_id)
            .or_insert_with(|| DeviceEventState::new());
        let event = state.process_event(&event);

        if let Some(event) = event {
            self.ui.device_event(app_ctrl, window_id, event);
        }
    }
}

/// Raw device event
impl<BackendMessage: Send + 'static> Application<BackendMessage> {
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
impl<BackendMessage: Send + 'static> Application<BackendMessage> {
    pub(crate) fn event_loop_commands(&self, cmd: ApplicationCommand) {
        todo!()
    }
}

/// User event
impl<BackendMessage: Send + 'static> Application<BackendMessage> {
    pub(crate) fn update_needed(&self, app_ctrl: &impl ApplicationControler) {
        todo!()
    }

    pub(crate) fn backend_message(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        msg: BackendMessage,
    ) {
        self.ui.user_event(app_ctrl, msg);
    }
}

/// Polling event
///
/// TODO: Wrap and abstract `winit::application::ApplicationHandler::new_events`
impl<BackendMessage: Send + 'static> Application<BackendMessage> {
    pub(crate) fn poll(&mut self, app_ctrl: &impl ApplicationControler) {
        todo!()
    }

    pub(crate) fn resume_time_reached(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        start: std::time::Instant,
        requested_resume: std::time::Instant,
    ) {
        self.ui.update(&self.window_manager, app_ctrl);
    }

    pub(crate) fn wait_cancelled(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        start: std::time::Instant,
        requested_resume: Option<std::time::Instant>,
    ) {
        // currently do nothing
    }
}

/// Currently not supported
impl<BackendMessage: Send + 'static> Application<BackendMessage> {
    pub(crate) fn about_to_wait(&mut self, app_ctrl: &impl ApplicationControler) {
        let _ = app_ctrl;
    }

    pub(crate) fn memory_warning(&mut self, app_ctrl: &impl ApplicationControler) {
        let _ = app_ctrl;
    }
}

// -------------------
// API type definition
// -------------------

pub(crate) trait ApplicationControler {}

pub(crate) enum ApplicationCommand {
    Exit,
}

pub(crate) trait ApplicationLoopProxy<BackendMessage: Send + 'static> {}
