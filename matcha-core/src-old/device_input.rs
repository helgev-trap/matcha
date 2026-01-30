pub mod button_state;
pub mod element_state;
pub mod key_input;
pub mod key_state;
pub mod mouse_input;
pub mod mouse_state;
pub mod window_state;

use std::path::PathBuf;

use button_state::ButtonState;
pub use element_state::ElementState;
pub use key_input::{Key, KeyCode, KeyInput, KeyLocation, ModifiersState, PhysicalKey};
pub use key_state::KeyboardState;
pub use mouse_input::MouseInput;
pub use mouse_input::MouseLogicalButton;
pub use mouse_state::MouseState;
pub use winit::window::Theme;

// MARK: Event

/// Represents a generic UI event within the application.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceInput {
    // raw event.
    raw: DeviceInputData,
    raw_winit: Option<winit::event::WindowEvent>,
    // information to calculate relative mouse position
    mouse_view_port_position: [f32; 2],
    left_multiplied_transform: nalgebra::Matrix4<f32>,
    left_multiplied_transform_inv: Option<nalgebra::Matrix4<f32>>,
    // relative event.
    relative: DeviceInputData,
}

/// constructor
impl DeviceInput {
    /// Creates a new `Event` from a `ConcreteEvent`.
    pub(crate) fn new(
        mouse_position: [f32; 2],
        event: DeviceInputData,
        raw_winit: Option<winit::event::WindowEvent>,
    ) -> Self {
        Self {
            raw: event.clone(),
            raw_winit,
            mouse_view_port_position: mouse_position,
            left_multiplied_transform: nalgebra::Matrix4::identity(),
            left_multiplied_transform_inv: Some(nalgebra::Matrix4::identity()),
            relative: event,
        }
    }

    pub fn transform(&self, child_affine: nalgebra::Matrix4<f32>) -> Self {
        let mut new = self.clone();
        new.left_multiplied_transform = self.left_multiplied_transform * child_affine;
        new.left_multiplied_transform_inv = new.left_multiplied_transform.try_inverse();
        new
    }

    pub fn with_custom_relative_input(mut self, relative: DeviceInputData) -> Self {
        self.relative = relative;
        self
    }
}

/// getter
impl DeviceInput {
    pub fn mouse_position(&self) -> Option<[f32; 2]> {
        let relative_position = self.left_multiplied_transform_inv?
            * nalgebra::Vector4::new(
                self.mouse_view_port_position[0],
                self.mouse_view_port_position[1],
                0.0,
                1.0,
            );
        Some([relative_position.x, relative_position.y])
    }

    pub fn raw_event(&self) -> &DeviceInputData {
        &self.raw
    }

    pub fn raw_winit(&self) -> Option<&winit::event::WindowEvent> {
        self.raw_winit.as_ref()
    }

    pub fn mouse_view_port_position(&self) -> [f32; 2] {
        self.mouse_view_port_position
    }

    pub fn event(&self) -> &DeviceInputData {
        &self.relative
    }
}

// todo: implement: on_drag_start / on_drag_end, on_focus / on_blur

/// Mouse click event
impl DeviceInput {
    // --- primary button click event ---

    pub fn on_click<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(u32) -> R,
    {
        match &self.relative {
            DeviceInputData::MouseInput {
                event:
                    Some(MouseInput::Click {
                        click_state: ElementState::Pressed(count),
                        button: MouseLogicalButton::Primary,
                    }),
                ..
            } => Some(f(*count)),
            _ => None,
        }
    }

    pub fn on_click_counted<F, R>(&self, count: u32, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        match &self.relative {
            DeviceInputData::MouseInput {
                event:
                    Some(MouseInput::Click {
                        click_state: ElementState::Pressed(c),
                        button: MouseLogicalButton::Primary,
                    }),
                ..
            } if *c == count => Some(f()),
            _ => None,
        }
    }

    pub fn on_long_press<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(u32) -> R,
    {
        match &self.relative {
            DeviceInputData::MouseInput {
                event:
                    Some(MouseInput::Click {
                        click_state: ElementState::LongPressed(count),
                        button: MouseLogicalButton::Primary,
                    }),
                ..
            } => Some(f(*count)),
            _ => None,
        }
    }

    pub fn on_click_released<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(u32) -> R,
    {
        match &self.relative {
            DeviceInputData::MouseInput {
                event:
                    Some(MouseInput::Click {
                        click_state: ElementState::Released(count),
                        button: MouseLogicalButton::Primary,
                    }),
                ..
            } => Some(f(*count)),
            _ => None,
        }
    }

    pub fn on_secondary_click<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(u32) -> R,
    {
        match &self.relative {
            DeviceInputData::MouseInput {
                event:
                    Some(MouseInput::Click {
                        click_state: ElementState::Pressed(count),
                        button: MouseLogicalButton::Secondary,
                    }),
                ..
            } => Some(f(*count)),
            _ => None,
        }
    }

    pub fn on_secondary_click_counted<F, R>(&self, count: u32, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        match &self.relative {
            DeviceInputData::MouseInput {
                event:
                    Some(MouseInput::Click {
                        click_state: ElementState::Pressed(c),
                        button: MouseLogicalButton::Secondary,
                    }),
                ..
            } if *c == count => Some(f()),
            _ => None,
        }
    }

    pub fn on_secondary_long_press<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(u32) -> R,
    {
        match &self.relative {
            DeviceInputData::MouseInput {
                event:
                    Some(MouseInput::Click {
                        click_state: ElementState::LongPressed(count),
                        button: MouseLogicalButton::Secondary,
                    }),
                ..
            } => Some(f(*count)),
            _ => None,
        }
    }

    pub fn on_secondary_click_released<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(u32) -> R,
    {
        match &self.relative {
            DeviceInputData::MouseInput {
                event:
                    Some(MouseInput::Click {
                        click_state: ElementState::Released(count),
                        button: MouseLogicalButton::Secondary,
                    }),
                ..
            } => Some(f(*count)),
            _ => None,
        }
    }

    pub fn on_middle_click<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(u32) -> R,
    {
        match &self.relative {
            DeviceInputData::MouseInput {
                event:
                    Some(MouseInput::Click {
                        click_state: ElementState::Pressed(count),
                        button: MouseLogicalButton::Middle,
                    }),
                ..
            } => Some(f(*count)),
            _ => None,
        }
    }

    pub fn on_mouse_button_event<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&MouseInput) -> R,
    {
        match &self.relative {
            DeviceInputData::MouseInput {
                event: Some(mouse_event),
                ..
            } => Some(f(mouse_event)),
            _ => None,
        }
    }
}

impl DeviceInput {
    pub fn on_mouse_enter<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        match &self.relative {
            DeviceInputData::MouseInput {
                event: Some(MouseInput::Entered),
                ..
            } => Some(f()),
            _ => None,
        }
    }

    pub fn on_mouse_leave<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        match &self.relative {
            DeviceInputData::MouseInput {
                event: Some(MouseInput::Left),
                ..
            } => Some(f()),
            _ => None,
        }
    }

    pub fn on_scroll<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce([f32; 2]) -> R,
    {
        match &self.relative {
            DeviceInputData::MouseInput {
                event: Some(MouseInput::Scroll { delta }),
                ..
            } => Some(f(*delta)),
            _ => None,
        }
    }

    pub fn on_drag<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce([f32; 2], MouseLogicalButton) -> R,
    {
        match &self.relative {
            DeviceInputData::MouseInput {
                dragging_from_primary: dragging_primary,
                dragging_from_secondary: dragging_secondary,
                dragging_from_middle: dragging_middle,
                ..
            } => {
                if let Some(pos) = dragging_primary {
                    return Some(f(*pos, MouseLogicalButton::Primary));
                }
                if let Some(pos) = dragging_secondary {
                    return Some(f(*pos, MouseLogicalButton::Secondary));
                }
                if let Some(pos) = dragging_middle {
                    return Some(f(*pos, MouseLogicalButton::Middle));
                }
                None
            }
            _ => None,
        }
    }

    pub fn on_key_down<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&KeyInput) -> R,
    {
        match &self.relative {
            DeviceInputData::Keyboard(key_input) => {
                if let ElementState::Pressed(_) = key_input.state() {
                    Some(f(key_input))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn on_key_up<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&KeyInput) -> R,
    {
        match &self.relative {
            DeviceInputData::Keyboard(key_input) => {
                if let ElementState::Released(_) = key_input.state() {
                    Some(f(key_input))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn on_file_drop<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&PathBuf) -> R,
    {
        match &self.relative {
            DeviceInputData::FileDrop { path_buf } => Some(f(path_buf)),
            _ => None,
        }
    }

    pub fn on_file_hover<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&PathBuf) -> R,
    {
        match &self.relative {
            DeviceInputData::FileHover { path_buf } => Some(f(path_buf)),
            _ => None,
        }
    }
}

/// Represents the concrete type of a UI event.
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceInputData {
    CloseRequested,
    WindowPositionSize {
        inner_position: [f32; 2],
        outer_position: [f32; 2],
        inner_size: [f32; 2],
        outer_size: [f32; 2],
    },
    WindowFocus(bool),
    FileDrop {
        path_buf: PathBuf,
    },
    FileHover {
        path_buf: PathBuf,
    },
    FileHoverCancelled,
    Keyboard(KeyInput),
    /// not implemented yet
    Ime,
    MouseInput {
        dragging_from_primary: Option<[f32; 2]>,
        dragging_from_secondary: Option<[f32; 2]>,
        dragging_from_middle: Option<[f32; 2]>,
        event: Option<MouseInput>,
    },
    /// not implemented yet
    Touch,
    Theme(Theme),
}
