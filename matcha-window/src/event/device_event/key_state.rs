use super::{DeviceEventData, KeyInput};
use std::collections::VecDeque;

pub use winit::keyboard::{Key, KeyCode, ModifiersState};

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct KeyboardState {
    press_order: VecDeque<(KeyCode, Key)>,
    modifiers: ModifiersState,
}

impl KeyboardState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn modifiers_changed(&mut self, modifiers: ModifiersState) {
        self.modifiers = modifiers;
    }

    /// Update internal state from `key_input`, fill `key_input.snapshot`, and return
    /// the resulting `DeviceEventData`.
    ///
    /// Returns `None` if `key_input.physical_key` is not a `PhysicalKey::Code` variant.
    pub fn keyboard_input(&mut self, key_input: &mut KeyInput) -> Option<DeviceEventData> {
        use super::ElementState;
        use super::key_input::PhysicalKey;

        let PhysicalKey::Code(key_code) = key_input.physical_key else {
            return None;
        };

        match key_input.state {
            ElementState::Pressed(_) => {
                self.press_order
                    .push_back((key_code, key_input.logical_key.clone()));
            }
            ElementState::Released(_) => {
                if let Some(pos) = self
                    .press_order
                    .iter()
                    .position(|(code, _)| *code == key_code)
                {
                    self.press_order.remove(pos);
                }
            }
            ElementState::LongPressed(_) => {}
        }

        key_input.snapshot = self.clone();

        Some(DeviceEventData::Keyboard(key_input.clone()))
    }
}

impl KeyboardState {
    pub fn is_physical_pressed(&self, key: &KeyCode) -> bool {
        self.press_order.iter().any(|(code, _)| code == key)
    }

    pub fn is_logical_pressed(&self, key: &Key) -> bool {
        self.press_order
            .iter()
            .any(|(_, logical_key)| logical_key == key)
    }

    pub fn modifiers(&self) -> ModifiersState {
        self.modifiers
    }

    pub fn press_order(&self) -> Vec<(KeyCode, Key)> {
        self.press_order.iter().cloned().collect()
    }
}
