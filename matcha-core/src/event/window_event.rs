/// A state that tracks window status like size, position, focus, etc.
#[derive(Debug, Default)]
pub struct WindowEventState {}

/// A high level window event.
#[derive(Debug, Clone)]
pub struct WindowEvent {}

impl WindowEventState {
    pub fn new() -> Self {
        todo!()
    }
}

impl WindowEventState {
    /// Process a winit event and update the internal state.
    /// Returns a high level WindowEvent if the event should be propagated to the UI.
    pub fn process_event(&mut self, event: &WindowEvent) -> Option<WindowEvent> {
        todo!()
    }
}
