use bevy::prelude::*;
use bytemuck::{Pod, Zeroable};

/// A [`Config`](`ggrs::Config`) compatible input designed to capture the state of [`Input<MouseButton>`].
#[derive(Copy, Clone, PartialEq, Eq, Zeroable, Pod, Default, Debug)]
#[repr(C)]
pub struct MouseButtonInput {
    mousebuttons: [u8; 3],
}

impl MouseButtonInput {
    fn map(mouse_button: MouseButton) -> (usize, u8) {
        let mouse_button_value = match mouse_button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            MouseButton::Other(value) => 3 + (value as u32),
        };

        let bank = (mouse_button_value / 8) as usize;
        let channel = (mouse_button_value % 8) as u8;

        (bank, channel)
    }

    pub fn get(&self, mouse_button: MouseButton) -> bool {
        let (bank, channel) = Self::map(mouse_button);

        let Some(&bank) = self.mousebuttons.get(bank) else {
            panic!(
                "MouseButtonInput is unable to operate on {:?}",
                mouse_button
            );
        };

        let mask = 1 << channel;

        bank & mask != 0
    }

    pub fn set(&mut self, mouse_button: MouseButton, value: bool) -> &mut Self {
        let (bank, channel) = Self::map(mouse_button);

        let Some(bank) = self.mousebuttons.get_mut(bank) else {
            panic!(
                "MouseButtonInput is unable to operate on {:?}",
                mouse_button
            );
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

impl From<&Input<MouseButton>> for MouseButtonInput {
    fn from(value: &Input<MouseButton>) -> Self {
        let mut input = MouseButtonInput::default();

        for &pressed in value.get_pressed() {
            input.set(pressed, true);
        }

        input
    }
}