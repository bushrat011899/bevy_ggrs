use std::marker::PhantomData;

use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::HashMap, window::PrimaryWindow};
use bytemuck::{Pod, Zeroable};
use ggrs::{Config, InputStatus, PlayerHandle};

use crate::LocalPlayers;

mod gamepad_button;
mod keycode;
mod mouse_button;
mod mouse_position;

pub use gamepad_button::*;
pub use keycode::*;
pub use mouse_button::*;
pub use mouse_position::*;

// TODO: more specific name to avoid conflicts?
#[derive(Resource, Deref, DerefMut)]
pub struct PlayerInputs<T: Config>(pub(crate) Vec<(T::Input, InputStatus)>);

/// Inputs from local players. You have to fill this resource in the ReadInputs schedule.
#[derive(Resource)]
pub struct LocalInputs<C: Config>(pub HashMap<PlayerHandle, C::Input>);

/// Label for the schedule which reads the inputs for the current frame
#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
pub struct ReadInputs;

/// A [`Config`] compatible input type which captures mouse and keyboard inputs.
#[derive(Copy, Clone, PartialEq, Eq, Zeroable, Pod, Default, Debug)]
#[repr(C)]
pub struct KeyboardAndMouseInput {
    pub mouse_position: MousePositionInput,
    pub mouse_buttons: MouseButtonInput,
    pub keyboard_buttons: KeyCodeInput,
}

impl KeyboardAndMouseInput {
    pub fn read_local_inputs<C>(
        mut commands: Commands,
        keyboard_input: Res<Input<KeyCode>>,
        mouse_input: Res<Input<MouseButton>>,
        windows: Query<&Window, With<PrimaryWindow>>,
        local_players: Res<LocalPlayers>,
    ) where
        C: Config<Input = KeyboardAndMouseInput>,
    {
        let keyboard_buttons = KeyCodeInput::from(keyboard_input.as_ref());
        let mouse_buttons = MouseButtonInput::from(mouse_input.as_ref());
        let mouse_position = windows
            .get_single()
            .map(MousePositionInput::from)
            .unwrap_or_default();

        let input = KeyboardAndMouseInput {
            keyboard_buttons,
            mouse_buttons,
            mouse_position,
        };

        let mut local_inputs = HashMap::new();

        for handle in &local_players.0 {
            local_inputs.insert(*handle, input.clone());
        }

        commands.insert_resource(LocalInputs::<C>(local_inputs));
    }
}

pub struct KeyboardAndMouseInputPlugin<C>
where
    C: Config<Input = KeyboardAndMouseInput>,
{
    _phantom: PhantomData<C>,
}

impl<C> Default for KeyboardAndMouseInputPlugin<C>
where
    C: Config<Input = KeyboardAndMouseInput>,
{
    fn default() -> Self {
        Self {
            _phantom: Default::default(),
        }
    }
}

impl<C> Plugin for KeyboardAndMouseInputPlugin<C>
where
    C: Config<Input = KeyboardAndMouseInput>,
{
    fn build(&self, app: &mut App) {
        app.add_systems(ReadInputs, KeyboardAndMouseInput::read_local_inputs::<C>);
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::App;

    use crate::{GgrsConfig, GgrsPlugin, KeyboardAndMouseInput};

    #[test]
    fn check_keyboard_and_mouse_input_is_valid() {
        type MyConfig = GgrsConfig<KeyboardAndMouseInput>;

        let mut app = App::new();

        app.add_plugins(GgrsPlugin::<MyConfig>::default());
    }
}
