use std::collections::HashSet;
use winit::dpi::PhysicalPosition;

/// Stateful processor for raw winit events.
///
/// This struct tracks the state of input devices (mouse, keyboard) for a single window
/// and produces higher-level `DeviceInput` events.
#[derive(Debug, Default)]
pub struct DeviceEventState {}

/// A processed input event with context.
#[derive(Debug, Clone)]
pub struct DeviceEvent {}

impl DeviceEventState {
    pub fn new() -> Self {
        todo!()
    }
}

impl DeviceEventState {
    /// Process a winit event and update the internal state.
    /// Returns a high level DeviceEvent if the event should be propagated to the UI.
    pub fn process_event(
        &mut self,
        event: &DeviceEvent,
    ) -> Option<DeviceEvent> {
        todo!()
    }
}

