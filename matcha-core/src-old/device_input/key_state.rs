use super::{DeviceInputData, KeyInput};
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

    pub fn keyboard_input(&mut self, key_event: winit::event::KeyEvent) -> Option<DeviceInputData> {
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
        Some(DeviceInputData::Keyboard(KeyInput {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use winit::keyboard::{Key, KeyCode, ModifiersState};

    #[test]
    fn default_state() {
        let ks = KeyboardState::new();
        assert_eq!(ks.modifiers(), ModifiersState::default());
        assert!(ks.press_order().is_empty());
        assert!(!ks.is_physical_pressed(&KeyCode::KeyA));
        assert!(!ks.is_logical_pressed(&Key::Character("a".into())));
    }

    #[test]
    fn modifiers_changed() {
        let mut ks = KeyboardState::new();
        let new_mods = ModifiersState::ALT | ModifiersState::SHIFT;
        ks.modifiers_changed(new_mods);
        assert_eq!(ks.modifiers(), new_mods);
    }

    #[test]
    fn press_order_and_queries() {
        let mut ks = KeyboardState::new();

        // Simulate keyboard state by directly manipulating press_order for testing
        // This tests the query methods without needing to construct winit events
        ks.press_order = VecDeque::from(vec![
            (KeyCode::KeyA, Key::Character("a".into())),
            (KeyCode::KeyB, Key::Character("b".into())),
        ]);

        // Test press_order method
        assert_eq!(
            ks.press_order(),
            vec![
                (KeyCode::KeyA, Key::Character("a".into())),
                (KeyCode::KeyB, Key::Character("b".into())),
            ]
        );

        // Test physical key queries
        assert!(ks.is_physical_pressed(&KeyCode::KeyA));
        assert!(ks.is_physical_pressed(&KeyCode::KeyB));
        assert!(!ks.is_physical_pressed(&KeyCode::KeyC));

        // Test logical key queries
        assert!(ks.is_logical_pressed(&Key::Character("a".into())));
        assert!(ks.is_logical_pressed(&Key::Character("b".into())));
        assert!(!ks.is_logical_pressed(&Key::Character("c".into())));
    }

    #[test]
    fn keyboard_input_state_management() {
        let mut ks = KeyboardState::new();

        // Since we can't easily construct WinitKeyEvent due to private fields,
        // we'll test the state management logic by simulating the internal behavior

        // Simulate pressing KeyA
        ks.press_order
            .push_back((KeyCode::KeyA, Key::Character("a".into())));
        assert!(ks.is_physical_pressed(&KeyCode::KeyA));
        assert!(ks.is_logical_pressed(&Key::Character("a".into())));
        assert_eq!(ks.press_order().len(), 1);

        // Simulate pressing KeyB
        ks.press_order
            .push_back((KeyCode::KeyB, Key::Character("b".into())));
        assert!(ks.is_physical_pressed(&KeyCode::KeyB));
        assert_eq!(ks.press_order().len(), 2);

        // Simulate releasing KeyA (should remove first occurrence)
        if let Some(pos) = ks
            .press_order
            .iter()
            .position(|(code, _)| *code == KeyCode::KeyA)
        {
            ks.press_order.remove(pos);
        }
        assert!(!ks.is_physical_pressed(&KeyCode::KeyA));
        assert!(ks.is_physical_pressed(&KeyCode::KeyB));
        assert_eq!(ks.press_order().len(), 1);

        // Simulate releasing KeyB
        if let Some(pos) = ks
            .press_order
            .iter()
            .position(|(code, _)| *code == KeyCode::KeyB)
        {
            ks.press_order.remove(pos);
        }
        assert!(!ks.is_physical_pressed(&KeyCode::KeyB));
        assert_eq!(ks.press_order().len(), 0);
    }

    #[test]
    fn press_order_maintains_sequence() {
        let mut ks = KeyboardState::new();

        // Simulate pressing keys in sequence: A, B, C
        let keys = [
            (KeyCode::KeyA, Key::Character("a".into())),
            (KeyCode::KeyB, Key::Character("b".into())),
            (KeyCode::KeyC, Key::Character("c".into())),
        ];

        for (physical, logical) in keys.iter() {
            ks.press_order.push_back((*physical, logical.clone()));
        }

        // Verify all keys are tracked and order is maintained
        assert_eq!(ks.press_order(), keys.to_vec());

        // Simulate releasing middle key (B)
        if let Some(pos) = ks
            .press_order
            .iter()
            .position(|(code, _)| *code == KeyCode::KeyB)
        {
            ks.press_order.remove(pos);
        }

        // Verify B is removed but A and C remain in order
        let expected = vec![
            (KeyCode::KeyA, Key::Character("a".into())),
            (KeyCode::KeyC, Key::Character("c".into())),
        ];
        assert_eq!(ks.press_order(), expected);
        assert!(!ks.is_physical_pressed(&KeyCode::KeyB));
        assert!(ks.is_physical_pressed(&KeyCode::KeyA));
        assert!(ks.is_physical_pressed(&KeyCode::KeyC));
    }

    #[test]
    fn duplicate_key_press_handling() {
        let mut ks = KeyboardState::new();

        // Simulate pressing the same key twice (like key repeat)
        ks.press_order
            .push_back((KeyCode::KeyA, Key::Character("a".into())));
        ks.press_order
            .push_back((KeyCode::KeyA, Key::Character("a".into())));

        // Should have two entries for the same key
        assert_eq!(ks.press_order().len(), 2);
        assert_eq!(
            ks.press_order(),
            vec![
                (KeyCode::KeyA, Key::Character("a".into())),
                (KeyCode::KeyA, Key::Character("a".into())),
            ]
        );

        // Key should still be considered pressed
        assert!(ks.is_physical_pressed(&KeyCode::KeyA));

        // Simulate releasing one instance (should remove first occurrence)
        if let Some(pos) = ks
            .press_order
            .iter()
            .position(|(code, _)| *code == KeyCode::KeyA)
        {
            ks.press_order.remove(pos);
        }

        // Should still have one instance and still be considered pressed
        assert_eq!(ks.press_order().len(), 1);
        assert!(ks.is_physical_pressed(&KeyCode::KeyA));
    }

    #[test]
    fn release_unpressed_key() {
        let ks = KeyboardState::new();

        // Try to release a key that was never pressed
        // The position search should return None and nothing should happen
        let pos = ks
            .press_order
            .iter()
            .position(|(code, _)| *code == KeyCode::KeyA);
        assert!(pos.is_none());

        // State should remain empty
        assert!(ks.press_order().is_empty());
        assert!(!ks.is_physical_pressed(&KeyCode::KeyA));
    }

    #[test]
    fn modifiers_integration() {
        let mut ks = KeyboardState::new();

        // Set modifiers first
        let modifiers = ModifiersState::CONTROL | ModifiersState::SHIFT;
        ks.modifiers_changed(modifiers);

        // Verify modifiers are stored correctly
        assert_eq!(ks.modifiers(), modifiers);

        // Simulate pressing a key with modifiers active
        ks.press_order
            .push_back((KeyCode::KeyA, Key::Character("A".into())));

        // Both the key state and modifiers should be tracked
        assert!(ks.is_physical_pressed(&KeyCode::KeyA));
        assert_eq!(ks.modifiers(), modifiers);
    }
}
