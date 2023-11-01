use bevy::prelude::*;
use bytemuck::{Pod, Zeroable};

/// A [`Config`](`ggrs::Config`) compatible input designed to capture the position of
/// the mouse cursor in the primary window, if available. For cross-client stability,
/// the cursor position is reported in normalized units, from `(0, 0)` -> `(1, 1)`.
#[derive(Copy, Clone, PartialEq, Eq, Zeroable, Pod, Debug)]
#[repr(C)]
pub struct MousePositionInput {
    x: [u8; 4],
    y: [u8; 4],
}

impl Default for MousePositionInput {
    fn default() -> Self {
        let mut input = Self {
            x: default(),
            y: default(),
        };

        input.set(None);

        input
    }
}

impl MousePositionInput {
    pub fn get(&self) -> Option<Vec2> {
        let x = f32::from_be_bytes(self.x);
        let y = f32::from_be_bytes(self.y);

        if !y.is_finite() || !x.is_finite() {
            return None;
        }

        Some(Vec2 { x, y })
    }

    pub fn set(&mut self, position: Option<Vec2>) -> &mut Self {
        let Vec2 { x, y } = position.unwrap_or(Vec2::new(f32::NAN, f32::NAN));

        self.x = x.to_be_bytes();
        self.y = y.to_be_bytes();

        self
    }
}

impl From<&Window> for MousePositionInput {
    fn from(value: &Window) -> Self {
        let mut input = MousePositionInput::default();

        let position = value.cursor_position().map(|Vec2 { x, y }| Vec2 {
            x: x / value.width() as f32,
            y: y / value.height() as f32,
        });

        input.set(position);

        input
    }
}