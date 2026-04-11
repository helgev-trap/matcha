// Internal implementation modules — not part of the public API.
// Types that consumers need are re-exported below via `pub use`.
mod button_state;
mod element_state;
mod key_input;
mod key_state;
mod mouse_input;
mod mouse_state;

use std::path::PathBuf;

use button_state::ButtonState;
use mouse_state::MouseState; // internal use only — not re-exported

// ----------------------------------------------------------------------------
// Public API re-exports
// ----------------------------------------------------------------------------
pub use element_state::ElementState;
pub use key_input::{Key, KeyCode, KeyInput, KeyLocation, ModifiersState, PhysicalKey};
pub use key_state::KeyboardState;
pub use mouse_input::{MouseInput, MouseLogicalButton};
/// Configuration for the mouse state machine.
/// Pass to [`DeviceEventState::new`] to customise combo / long-press timings.
pub use mouse_state::{MousePrimaryButton, MouseStateConfig};

// ----------------------------------------------------------------------------
// DeviceEventState
// ----------------------------------------------------------------------------

/// Stateful processor for raw winit events.
///
/// This struct tracks the state of input devices (mouse, keyboard) for a single window
/// and produces higher-level `DeviceEvent` events.
#[derive(Debug)]
pub struct DeviceEventState {
    mouse: MouseState,
    keyboard: KeyboardState,
}

impl DeviceEventState {
    /// Creates a `DeviceEventState` with a custom mouse configuration.
    ///
    /// Returns `None` when the configuration is invalid
    /// (i.e. `combo_duration > long_press_duration`).
    pub fn new(config: MouseStateConfig) -> Option<Self> {
        Some(Self {
            mouse: config.init()?,
            keyboard: KeyboardState::new(),
        })
    }
}

impl Default for DeviceEventState {
    fn default() -> Self {
        Self::new(MouseStateConfig::default())
            .expect("default DeviceEventState config is always valid")
    }
}

impl DeviceEventState {
    /// Handle `winit::event::WindowEvent::CursorMoved`.
    pub fn cursor_moved(
        &mut self,
        position: winit::dpi::PhysicalPosition<f64>,
    ) -> DeviceEvent {
        let data = self.mouse.cursor_moved(position);
        DeviceEvent {
            raw: data.clone(),
            mouse_viewport_position: self.mouse.position(),
            left_multiplied_transform: nalgebra::Matrix4::identity(),
            left_multiplied_transform_inv: Some(nalgebra::Matrix4::identity()),
            relative: data,
        }
    }

    /// Handle `winit::event::WindowEvent::CursorEntered`.
    pub fn cursor_entered(&self) -> DeviceEvent {
        let data = self.mouse.cursor_entered();
        DeviceEvent {
            raw: data.clone(),
            mouse_viewport_position: self.mouse.position(),
            left_multiplied_transform: nalgebra::Matrix4::identity(),
            left_multiplied_transform_inv: Some(nalgebra::Matrix4::identity()),
            relative: data,
        }
    }

    /// Handle `winit::event::WindowEvent::CursorLeft`.
    pub fn cursor_left(&self) -> DeviceEvent {
        let data = self.mouse.cursor_left();
        DeviceEvent {
            raw: data.clone(),
            mouse_viewport_position: self.mouse.position(),
            left_multiplied_transform: nalgebra::Matrix4::identity(),
            left_multiplied_transform_inv: Some(nalgebra::Matrix4::identity()),
            relative: data,
        }
    }

    /// Handle `winit::event::WindowEvent::MouseWheel`.
    pub fn mouse_wheel(&self, delta: winit::event::MouseScrollDelta) -> DeviceEvent {
        let data = self.mouse.mouse_wheel(delta);
        DeviceEvent {
            raw: data.clone(),
            mouse_viewport_position: self.mouse.position(),
            left_multiplied_transform: nalgebra::Matrix4::identity(),
            left_multiplied_transform_inv: Some(nalgebra::Matrix4::identity()),
            relative: data,
        }
    }

    /// Handle `winit::event::WindowEvent::MouseInput`.
    pub fn mouse_input(
        &mut self,
        button: winit::event::MouseButton,
        state: winit::event::ElementState,
    ) -> Option<DeviceEvent> {
        let data = self.mouse.mouse_input(button, state)?;
        Some(DeviceEvent {
            raw: data.clone(),
            mouse_viewport_position: self.mouse.position(),
            left_multiplied_transform: nalgebra::Matrix4::identity(),
            left_multiplied_transform_inv: Some(nalgebra::Matrix4::identity()),
            relative: data,
        })
    }

    /// Handle `winit::event::WindowEvent::KeyboardInput`.
    pub fn keyboard_input(&mut self, key_event: winit::event::KeyEvent) -> Option<DeviceEvent> {
        let data = self.keyboard.keyboard_input(key_event)?;
        Some(DeviceEvent {
            raw: data.clone(),
            mouse_viewport_position: self.mouse.position(),
            left_multiplied_transform: nalgebra::Matrix4::identity(),
            left_multiplied_transform_inv: Some(nalgebra::Matrix4::identity()),
            relative: data,
        })
    }

    /// Handle `winit::event::WindowEvent::ModifiersChanged`.
    pub fn modifiers_changed(&mut self, modifiers: winit::keyboard::ModifiersState) {
        self.keyboard.modifiers_changed(modifiers);
    }

    /// Handle `winit::event::WindowEvent::DroppedFile`.
    pub fn dropped_file(&self, path_buf: PathBuf) -> DeviceEvent {
        let data = DeviceEventData::FileDrop { path_buf };
        DeviceEvent {
            raw: data.clone(),
            mouse_viewport_position: self.mouse.position(),
            left_multiplied_transform: nalgebra::Matrix4::identity(),
            left_multiplied_transform_inv: Some(nalgebra::Matrix4::identity()),
            relative: data,
        }
    }

    /// Handle `winit::event::WindowEvent::HoveredFile`.
    pub fn hovered_file(&self, path_buf: PathBuf) -> DeviceEvent {
        let data = DeviceEventData::FileHover { path_buf };
        DeviceEvent {
            raw: data.clone(),
            mouse_viewport_position: self.mouse.position(),
            left_multiplied_transform: nalgebra::Matrix4::identity(),
            left_multiplied_transform_inv: Some(nalgebra::Matrix4::identity()),
            relative: data,
        }
    }

    /// Handle `winit::event::WindowEvent::HoveredFileCancelled`.
    pub fn hovered_file_cancelled(&self) -> DeviceEvent {
        let data = DeviceEventData::FileHoverCancelled;
        DeviceEvent {
            raw: data.clone(),
            mouse_viewport_position: self.mouse.position(),
            left_multiplied_transform: nalgebra::Matrix4::identity(),
            left_multiplied_transform_inv: Some(nalgebra::Matrix4::identity()),
            relative: data,
        }
    }

    /// Detect long presses for all mouse buttons.
    /// Should be called on every frame/poll cycle.
    pub fn long_pressing_detection(&mut self) -> Vec<DeviceEvent> {
        self.mouse
            .long_pressing_detection()
            .into_iter()
            .map(|data| DeviceEvent {
                raw: data.clone(),
                mouse_viewport_position: self.mouse.position(),
                left_multiplied_transform: nalgebra::Matrix4::identity(),
                left_multiplied_transform_inv: Some(nalgebra::Matrix4::identity()),
                relative: data,
            })
            .collect()
    }
}

// ----------------------------------------------------------------------------
// DeviceEvent
// ----------------------------------------------------------------------------

/// A processed input event with context (mouse position, transform, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceEvent {
    // raw event data
    raw: DeviceEventData,
    // current mouse viewport position
    mouse_viewport_position: [f32; 2],
    // accumulated left-multiplied transform for coordinate conversion
    left_multiplied_transform: nalgebra::Matrix4<f32>,
    left_multiplied_transform_inv: Option<nalgebra::Matrix4<f32>>,
    // event data transformed into the local coordinate system
    relative: DeviceEventData,
}

impl DeviceEvent {
    /// Apply a child widget's affine transform to produce a new event in that widget's
    /// local coordinate space.
    pub fn transform(&self, child_affine: nalgebra::Matrix4<f32>) -> Self {
        let mut new = self.clone();
        new.left_multiplied_transform = self.left_multiplied_transform * child_affine;
        new.left_multiplied_transform_inv = new.left_multiplied_transform.try_inverse();
        new
    }

    /// Override the relative event data (useful for custom hit-testing).
    pub fn with_custom_relative(mut self, relative: DeviceEventData) -> Self {
        self.relative = relative;
        self
    }
}

/// Getters
impl DeviceEvent {
    /// Returns the mouse position in the local coordinate space, or `None` if the
    /// transform is not invertible.
    pub fn mouse_position(&self) -> Option<[f32; 2]> {
        let relative_position = self.left_multiplied_transform_inv?
            * nalgebra::Vector4::new(
                self.mouse_viewport_position[0],
                self.mouse_viewport_position[1],
                0.0,
                1.0,
            );
        Some([relative_position.x, relative_position.y])
    }

    /// Returns the raw (untransformed) event data.
    pub fn raw_event(&self) -> &DeviceEventData {
        &self.raw
    }

    /// Returns the mouse viewport position in the original screen space.
    pub fn mouse_viewport_position(&self) -> [f32; 2] {
        self.mouse_viewport_position
    }

    /// Returns the event data in the local coordinate space.
    pub fn event(&self) -> &DeviceEventData {
        &self.relative
    }
}

// Mouse click convenience methods
impl DeviceEvent {
    pub fn on_click<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(u32) -> R,
    {
        match &self.relative {
            DeviceEventData::MouseInput {
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
            DeviceEventData::MouseInput {
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
            DeviceEventData::MouseInput {
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
            DeviceEventData::MouseInput {
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
            DeviceEventData::MouseInput {
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
            DeviceEventData::MouseInput {
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
            DeviceEventData::MouseInput {
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
            DeviceEventData::MouseInput {
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
            DeviceEventData::MouseInput {
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
            DeviceEventData::MouseInput {
                event: Some(mouse_event),
                ..
            } => Some(f(mouse_event)),
            _ => None,
        }
    }
}

// Mouse cursor / scroll / drag convenience methods
impl DeviceEvent {
    pub fn on_mouse_enter<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        match &self.relative {
            DeviceEventData::MouseInput {
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
            DeviceEventData::MouseInput {
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
            DeviceEventData::MouseInput {
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
            DeviceEventData::MouseInput {
                dragging_from_primary,
                dragging_from_secondary,
                dragging_from_middle,
                ..
            } => {
                if let Some(pos) = dragging_from_primary {
                    return Some(f(*pos, MouseLogicalButton::Primary));
                }
                if let Some(pos) = dragging_from_secondary {
                    return Some(f(*pos, MouseLogicalButton::Secondary));
                }
                if let Some(pos) = dragging_from_middle {
                    return Some(f(*pos, MouseLogicalButton::Middle));
                }
                None
            }
            _ => None,
        }
    }
}

// Keyboard convenience methods
impl DeviceEvent {
    pub fn on_key_down<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&KeyInput) -> R,
    {
        match &self.relative {
            DeviceEventData::Keyboard(key_input) => {
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
            DeviceEventData::Keyboard(key_input) => {
                if let ElementState::Released(_) = key_input.state() {
                    Some(f(key_input))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

// File drag and drop convenience methods
impl DeviceEvent {
    pub fn on_file_drop<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&PathBuf) -> R,
    {
        match &self.relative {
            DeviceEventData::FileDrop { path_buf } => Some(f(path_buf)),
            _ => None,
        }
    }

    pub fn on_file_hover<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&PathBuf) -> R,
    {
        match &self.relative {
            DeviceEventData::FileHover { path_buf } => Some(f(path_buf)),
            _ => None,
        }
    }
}

// ----------------------------------------------------------------------------
// DeviceEventData
// ----------------------------------------------------------------------------

/// The concrete payload of a device event.
///
/// Note: Window-level events (close request, resize, focus, theme, …) are handled
/// separately by [`crate::event::window_event::WindowEvent`].
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceEventData {
    FileDrop {
        path_buf: PathBuf,
    },
    FileHover {
        path_buf: PathBuf,
    },
    FileHoverCancelled,
    Keyboard(KeyInput),
    /// Not implemented yet.
    Ime,
    MouseInput {
        dragging_from_primary: Option<[f32; 2]>,
        dragging_from_secondary: Option<[f32; 2]>,
        dragging_from_middle: Option<[f32; 2]>,
        event: Option<MouseInput>,
    },
    /// Not implemented yet.
    Touch,
}
