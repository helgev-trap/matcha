use std::any::Any;

use crate::application::ApplicationControler;

use crate::event::device_event::{DeviceEvent, DeviceEventState};
use crate::event::raw_device_event::{RawDeviceEvent, RawDeviceId};
use crate::event::window_event::{WindowEvent, WindowEventState};

use crate::window::{WindowConfig, WindowId};
use crate::window_manager::WindowManager;

pub mod component;
pub mod metrics;
pub mod update_flag;
pub mod widget;

// -------
// Globals
// -------

/// Initial value is true to force the first rendering.
static COMPONENT_RE_VIEW_NEEDED: update_flag::UpdateFlags = update_flag::UpdateFlags::new_true();

impl update_flag::UpdateFlags {
    // Intentionally empty — the inherent impl lives in update_flag.rs.
    // Access `COMPONENT_RE_VIEW_NEEDED.wakeup_handle()` to get a handle
    // that background tasks can use to signal the event loop.
}

/// Returns a [`WakeupHandle`](update_flag::WakeupHandle) backed by the global
/// `COMPONENT_RE_VIEW_NEEDED` flag.
///
/// Pass this to [`ComponentPod::set_wakeup`] during initialization so that
/// tasks spawned via [`TaskHandler::spawn_msg`] can wake the event loop when
/// their result is ready.
pub(crate) fn global_wakeup_handle() -> update_flag::WakeupHandle {
    COMPONENT_RE_VIEW_NEEDED.wakeup_handle()
}

// ------
// UiArch
// ------

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
    // Called on every event-loop iteration (winit AboutToWait / MainEventsCleared).
    // Drains pending task messages and rebuilds the view tree if anything changed.
    pub(crate) fn update(
        &mut self,
        window_manager: &WindowManager,
        app_ctrl: &impl ApplicationControler,
    ) {
        if !COMPONENT_RE_VIEW_NEEDED.value() {
            return;
        }
        COMPONENT_RE_VIEW_NEEDED.clear();

        // TODO: For each ComponentPod managed by this UiArch:
        //
        //   let ctx = ...; // construct a WidgetContext
        //   if component_pod.poll_messages(&ctx) {
        //       // At least one background task delivered a message.
        //       // Call component_pod.view(&ctx) and diff / apply to the widget tree.
        //   }
        //
        // This is wired up once ComponentPod storage is added to UiArch.
    }
}
