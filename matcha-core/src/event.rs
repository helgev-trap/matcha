pub mod device_event;
pub mod raw_device_event;
pub mod window_event;

use std::collections::HashMap;

use crate::{
    event::{device_event::DeviceEvent, window_event::WindowEvent},
    window::WindowId,
};

use fxhash::FxBuildHasher;

pub struct AllWindowStates {
    windows: HashMap<WindowId, WindowState, FxBuildHasher>,
}

pub struct WindowState {
    // todo
}

impl AllWindowStates {
    pub fn new() -> Self {
        Self {
            windows: HashMap::default(),
        }
    }

    /// Call this when a new window is created, to ensure old window state is cleaned up,
    /// in case a duplicated WindowId is used.
    pub fn new_window(&mut self, window_id: WindowId) {
        todo!()
    }

    // This is not supposed to be used because it will be very complicated
    // to implement RAII-Sync between Window and WindowState.
    // pub fn remove_window(&mut self, window_id: WindowId) {
    //     todo!()
    // }
}

impl AllWindowStates {
    pub fn process_window_event(
        &mut self,
        window_id: WindowId,
        event: &WindowEvent,
    ) -> Option<WindowEvent> {
        todo!()
    }

    pub fn process_device_event(
        &mut self,
        window_id: WindowId,
        event: &DeviceEvent,
    ) -> Option<DeviceEvent> {
        todo!()
    }
}
