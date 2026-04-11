use super::ElementState;

/// Represents a logical mouse button, abstracting away the physical primary/secondary mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseLogicalButton {
    Primary,
    Secondary,
    Middle,
    Back,
    Forward,
}

/// Represents a specific type of mouse event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseInput {
    Click {
        click_state: ElementState,
        button: MouseLogicalButton,
    },
    Entered,
    Left,
    Scroll {
        delta: [f32; 2],
    },
}
