/// Represents the state of an element (e.g., a key or button).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementState {
    /// The element is pressed, with an associated count (e.g., repeat count, click combo).
    Pressed(u32),
    /// The element has been long-pressed, with an associated count.
    LongPressed(u32),
    /// The element has been released, with an associated count.
    Released(u32),
}

impl ElementState {
    /// Converts a winit `ElementState` to the application's `ElementState`, including a count.
    pub(crate) fn from_winit_state_with_count(
        state: winit::event::ElementState,
        count: u32,
    ) -> Self {
        match state {
            winit::event::ElementState::Pressed => ElementState::Pressed(count),
            winit::event::ElementState::Released => ElementState::Released(count),
        }
    }
}

impl From<winit::event::ElementState> for ElementState {
    fn from(state: winit::event::ElementState) -> Self {
        ElementState::from_winit_state_with_count(state, 1)
    }
}
