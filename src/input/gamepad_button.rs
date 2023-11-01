use bevy::prelude::*;
use bytemuck::{Pod, Zeroable};

/// A [`Config`](`ggrs::Config`) compatible input designed to capture the state of [`Input<GamepadButton>`].
#[derive(Copy, Clone, PartialEq, Eq, Zeroable, Pod, Default, Debug)]
#[repr(C)]
pub struct GamepadButtonInput {
    gamepadbuttons: [u8; 32],
}

impl GamepadButtonInput {
    fn map(gamepad_button: GamepadButton) -> (usize, u8) {
        const MAX_GAMEPADS: u32 = 4;
        const MAX_BUTTONS: u32 = 64;

        let gamepad = gamepad_button.gamepad.id as u32;

        debug_assert!(
            gamepad < MAX_GAMEPADS,
            "GamepadButtonInput is unable to operate on {:?}",
            gamepad_button.gamepad
        );

        let gamepad_button_value = match gamepad_button.button_type {
            GamepadButtonType::South => 0,
            GamepadButtonType::East => 1,
            GamepadButtonType::North => 2,
            GamepadButtonType::West => 3,
            GamepadButtonType::C => 4,
            GamepadButtonType::Z => 5,
            GamepadButtonType::LeftTrigger => 6,
            GamepadButtonType::LeftTrigger2 => 7,
            GamepadButtonType::RightTrigger => 8,
            GamepadButtonType::RightTrigger2 => 9,
            GamepadButtonType::Select => 10,
            GamepadButtonType::Start => 11,
            GamepadButtonType::Mode => 12,
            GamepadButtonType::LeftThumb => 13,
            GamepadButtonType::RightThumb => 14,
            GamepadButtonType::DPadUp => 15,
            GamepadButtonType::DPadDown => 16,
            GamepadButtonType::DPadLeft => 17,
            GamepadButtonType::DPadRight => 18,
            GamepadButtonType::Other(other) => 19 + (other as u32),
        };

        debug_assert!(
            gamepad_button_value < MAX_BUTTONS,
            "GamepadButtonInput is unable to operate on {:?}",
            gamepad_button.button_type
        );

        let gamepad_button_value = gamepad_button_value + MAX_BUTTONS * gamepad;

        let bank = (gamepad_button_value / 8) as usize;
        let channel = (gamepad_button_value % 8) as u8;

        (bank, channel)
    }

    pub fn get(&self, gamepad_button: GamepadButton) -> bool {
        let (bank, channel) = Self::map(gamepad_button);

        let Some(&bank) = self.gamepadbuttons.get(bank) else {
            panic!(
                "GamepadButtonInput is unable to operate on {:?}",
                gamepad_button
            );
        };

        let mask = 1 << channel;

        bank & mask != 0
    }

    pub fn set(&mut self, gamepad_button: GamepadButton, value: bool) -> &mut Self {
        let (bank, channel) = Self::map(gamepad_button);

        let Some(bank) = self.gamepadbuttons.get_mut(bank) else {
            panic!(
                "GamepadButtonInput is unable to operate on {:?}",
                gamepad_button
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

impl From<&Input<GamepadButton>> for GamepadButtonInput {
    fn from(value: &Input<GamepadButton>) -> Self {
        let mut input = GamepadButtonInput::default();

        for &pressed in value.get_pressed() {
            input.set(pressed, true);
        }

        input
    }
}