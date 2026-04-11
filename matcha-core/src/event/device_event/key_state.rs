use super::{DeviceEventData, KeyInput};
use std::collections::VecDeque;

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct KeyboardState {
    press_order: VecDeque<(winit::keyboard::KeyCode, winit::keyboard::Key)>,
    modifiers: winit::keyboard::ModifiersState,
}

// `KeyboardState` assumes winit issues `ModifiersChanged` followed by `KeyboardInput`.
impl KeyboardState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn modifiers_changed(&mut self, modifiers: winit::keyboard::ModifiersState) {
        self.modifiers = modifiers;
    }

    pub fn keyboard_input(&mut self, key_event: winit::event::KeyEvent) -> Option<DeviceEventData> {
        let winit::keyboard::PhysicalKey::Code(key_code) = key_event.physical_key else {
            return None;
        };
        match key_event.state {
            winit::event::ElementState::Pressed => {
                self.press_order
                    .push_back((key_code, key_event.logical_key.clone()));
            }
            winit::event::ElementState::Released => {
                if let Some(pos) = self
                    .press_order
                    .iter()
                    .position(|(code, _)| *code == key_code)
                {
                    self.press_order.remove(pos);
                }
            }
        }
        Some(DeviceEventData::Keyboard(KeyInput {
            winit: key_event,
            snapshot: self.clone(),
        }))
    }
}

impl KeyboardState {
    pub fn is_physical_pressed(&self, key: &winit::keyboard::KeyCode) -> bool {
        self.press_order.iter().any(|(code, _)| code == key)
    }

    pub fn is_logical_pressed(&self, key: &winit::keyboard::Key) -> bool {
        self.press_order
            .iter()
            .any(|(_, logical_key)| logical_key == key)
    }

    pub fn modifiers(&self) -> winit::keyboard::ModifiersState {
        self.modifiers
    }

    pub fn press_order(&self) -> Vec<(winit::keyboard::KeyCode, winit::keyboard::Key)> {
        self.press_order.iter().cloned().collect()
    }
}
