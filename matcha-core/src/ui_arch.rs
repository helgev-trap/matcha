use std::any::Any;

use crate::application::ApplicationControler;

use crate::event::device_event::{DeviceEvent, DeviceEventState};
use crate::event::raw_device_event::{RawDeviceEvent, RawDeviceId};
use crate::event::window_event::{WindowEvent, WindowEventState};

use crate::window::{WindowConfig, WindowId};
use crate::window_manager::WindowManager;

pub mod metrics;
pub mod component;
pub mod widget;

pub struct UiArch<BackendMessage> {
    _phantom: std::marker::PhantomData<BackendMessage>,
}

/// Lifecycle methods
impl<BackendMessage> UiArch<BackendMessage> {
    // Called when the application is initialized
    pub(crate) fn init(&mut self, app_ctrl: &impl ApplicationControler) {
        todo!()
    }

    // Called when the application is resumed
    pub(crate) fn resumed(&mut self, app_ctrl: &impl ApplicationControler) {}

    // Called when the application is suspended
    pub(crate) fn suspended(&mut self, app_ctrl: &impl ApplicationControler) {}

    // Called when the application is exiting
    pub(crate) fn exiting(&mut self, app_ctrl: &impl ApplicationControler) {}
}

/// GPU Device Lost
impl<BackendMessage> UiArch<BackendMessage> {
    /// MUST INVALIDATE ALL GPU RESOURCES
    pub(crate) fn gpu_device_lost(&mut self, app_ctrl: &impl ApplicationControler) {}
}

/// Event handlers
impl<BackendMessage> UiArch<BackendMessage> {
    // Called when a window event is received (resize, input, close requested, etc.)
    // Returns `true` if the event was consumed, `false` otherwise.
    pub(crate) fn window_event(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        window_id: WindowId,
        event: WindowEvent,
    ) -> bool {
        false
    }

    pub(crate) fn device_event(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        window_id: WindowId,
        event: DeviceEvent,
    ) -> bool {
        false
    }

    pub(crate) fn raw_device_event(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        raw_device_id: RawDeviceId,
        raw_event: RawDeviceEvent,
    ) {
    }

    // Called when a custom user event is received (e.g. from background task)
    pub(crate) fn user_event(
        &mut self,
        app_ctrl: &impl ApplicationControler,
        event: BackendMessage,
    ) {
    }
}

/// UI update methods
impl<BackendMessage> UiArch<BackendMessage> {
    // Called when the application needs to update
    pub(crate) fn update(
        &mut self,
        window_manager: &WindowManager,
        app_ctrl: &impl ApplicationControler,
    ) {
    }
}
