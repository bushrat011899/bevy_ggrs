use bevy::prelude::*;
use bytemuck::{Pod, Zeroable};

/// A [`Config`](`ggrs::Config`) compatible input designed to capture the state of [`Input<KeyCode>`].
#[derive(Copy, Clone, PartialEq, Eq, Zeroable, Pod, Default, Debug)]
#[repr(C)]
pub struct KeyCodeInput {
    keycodes: [u8; 21],
}

impl KeyCodeInput {
    fn map(keycode: KeyCode) -> (usize, u8) {
        let keycode_value = keycode as u32;
        let channel = keycode_value % 8;
        let bank = (keycode_value - channel) / 8;
        (bank as usize, channel as u8)
    }

    pub fn get(&self, keycode: KeyCode) -> bool {
        let (bank, channel) = Self::map(keycode);

        let Some(&bank) = self.keycodes.get(bank) else {
            panic!("KeyCodeInput is unable to operate on {:?}", keycode);
        };

        let mask = 1 << channel;

        bank & mask != 0
    }

    pub fn set(&mut self, keycode: KeyCode, value: bool) -> &mut Self {
        let (bank, channel) = Self::map(keycode);

        let Some(bank) = self.keycodes.get_mut(bank) else {
            panic!("KeyCodeInput is unable to operate on {:?}", keycode);
        };

        let mask = 1 << channel;

        if value {
            *bank |= mask;
        } else {
            *bank &= !mask;
        }

        self
    }
}

impl From<&Input<KeyCode>> for KeyCodeInput {
    fn from(value: &Input<KeyCode>) -> Self {
        let mut input = KeyCodeInput::default();

        for &pressed in value.get_pressed() {
            input.set(pressed, true);
        }

        input
    }
}