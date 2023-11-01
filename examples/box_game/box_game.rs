use bevy::{prelude::*, window::PrimaryWindow};
use bevy_ggrs::{
    AddRollbackCommandExtension, GgrsConfig, KeyboardAndMouseInput, PlayerInputs, Rollback, Session,
};
use std::hash::Hash;

const BLUE: Color = Color::rgb(0.8, 0.6, 0.2);
const ORANGE: Color = Color::rgb(0., 0.35, 0.8);
const MAGENTA: Color = Color::rgb(0.9, 0.2, 0.2);
const GREEN: Color = Color::rgb(0.35, 0.7, 0.35);
const PLAYER_COLORS: [Color; 4] = [BLUE, ORANGE, MAGENTA, GREEN];

const INPUT_UP: KeyCode = KeyCode::W;
const INPUT_DOWN: KeyCode = KeyCode::S;
const INPUT_LEFT: KeyCode = KeyCode::A;
const INPUT_RIGHT: KeyCode = KeyCode::D;

const MOVEMENT_SPEED: f32 = 0.005;
const MAX_SPEED: f32 = 0.05;
const FRICTION: f32 = 0.9;
const PLANE_SIZE: f32 = 5.0;
const CUBE_SIZE: f32 = 0.2;

// You need to define a config struct to bundle all the generics of GGRS. bevy_ggrs provides a sensible default in `GgrsConfig`.
// (optional) You can define a type here for brevity.
pub type BoxConfig = GgrsConfig<KeyboardAndMouseInput>;

#[derive(Default, Component)]
pub struct Player {
    pub handle: usize,
}

// Components that should be saved/loaded need to implement the `Reflect` trait
#[derive(Default, Reflect, Component, Clone)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

// You can also register resources.
#[derive(Resource, Default, Reflect, Hash, Clone, Copy)]
#[reflect(Hash)]
pub struct FrameCount {
    pub frame: u32,
}

#[derive(Component)]
pub struct Ground;

pub fn setup_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    session: Res<Session<BoxConfig>>,
) {
    let num_players = match &*session {
        Session::SyncTest(s) => s.num_players(),
        Session::P2P(s) => s.num_players(),
        Session::Spectator(s) => s.num_players(),
    };

    // plane
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Plane {
                size: PLANE_SIZE,
                ..default()
            })),
            material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
            ..default()
        },
        Ground,
    ));

    // player cube - just spawn whatever entity you want, then add a `Rollback` component with a unique id (for example through the `RollbackIdProvider` resource).
    // Every entity that you want to be saved/loaded needs a `Rollback` component with a unique rollback id.
    // When loading entities from the past, this extra id is necessary to connect entities over different game states
    let r = PLANE_SIZE / 4.;

    for handle in 0..num_players {
        let rot = handle as f32 / num_players as f32 * 2. * std::f32::consts::PI;
        let x = r * rot.cos();
        let z = r * rot.sin();

        let mut transform = Transform::default();
        transform.translation.x = x;
        transform.translation.y = CUBE_SIZE / 2.;
        transform.translation.z = z;
        let color = PLAYER_COLORS[handle % PLAYER_COLORS.len()];

        commands
            .spawn((
                PbrBundle {
                    mesh: meshes.add(Mesh::from(shape::Cube { size: CUBE_SIZE })),
                    material: materials.add(color.into()),
                    transform,
                    ..default()
                },
                Player { handle },
                Velocity::default(),
            ))
            .add_rollback();
    }

    // light
    commands.spawn(PointLightBundle {
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });
    // camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 7.5, 0.5).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}

// Example system, manipulating a resource, will be added to the rollback schedule.
// Increases the frame count by 1 every update step. If loading and saving resources works correctly,
// you should see this resource rolling back, counting back up and finally increasing by 1 every update step
#[allow(dead_code)]
pub fn increase_frame_system(mut frame_count: ResMut<FrameCount>) {
    frame_count.frame += 1;
}

pub fn look_at_cursor(
    mut query: Query<(&mut Transform, &Player), With<Rollback>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    ground_query: Query<&GlobalTransform, With<Ground>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    inputs: Res<PlayerInputs<BoxConfig>>,
) {
    let window = windows.single();
    let window_scale = Vec2::new(window.width(), window.height());
    let (camera, camera_transform) = camera_query.single();
    let ground = ground_query.single();

    for (mut t, p) in query.iter_mut() {
        let input = inputs[p.handle].0;

        let Some(cursor_position_normalised) = input.mouse_position.get() else {
            continue;
        };

        let cursor_position = cursor_position_normalised * window_scale;

        // Calculate a ray pointing from the camera into the world based on the cursor's position.
        let Some(ray) = camera.viewport_to_world(camera_transform, cursor_position) else {
            continue;
        };

        // Calculate if and where the ray is hitting the ground plane.
        let Some(distance) = ray.intersect_plane(ground.translation(), ground.up()) else {
            continue;
        };

        let point = ray.get_point(distance);

        t.look_at(point, ground.up());
    }
}

// Example system that moves the cubes, will be added to the rollback schedule.
// Filtering for the rollback component is a good way to make sure your game logic systems
// only mutate components that are being saved/loaded.
#[allow(dead_code)]
pub fn move_cube_system(
    mut query: Query<(&mut Transform, &mut Velocity, &Player), With<Rollback>>,
    inputs: Res<PlayerInputs<BoxConfig>>,
) {
    for (mut t, mut v, p) in query.iter_mut() {
        let input = inputs[p.handle].0;

        let up = input.keyboard_buttons.get(INPUT_UP);
        let down = input.keyboard_buttons.get(INPUT_DOWN);
        let left = input.keyboard_buttons.get(INPUT_LEFT);
        let right = input.keyboard_buttons.get(INPUT_RIGHT);

        // set velocity through key presses
        if up && !down {
            v.z -= MOVEMENT_SPEED;
        }
        if !up && down {
            v.z += MOVEMENT_SPEED;
        }
        if left && !right {
            v.x -= MOVEMENT_SPEED;
        }
        if !left && right {
            v.x += MOVEMENT_SPEED;
        }

        // slow down
        if !up && !down {
            v.z *= FRICTION;
        }
        if !left && !right {
            v.x *= FRICTION;
        }
        v.y *= FRICTION;

        // constrain velocity
        let mag = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
        if mag > MAX_SPEED {
            let factor = MAX_SPEED / mag;
            v.x *= factor;
            v.y *= factor;
            v.z *= factor;
        }

        // apply velocity
        t.translation.x += v.x;
        t.translation.y += v.y;
        t.translation.z += v.z;

        // constrain cube to plane
        t.translation.x = t.translation.x.max(-1. * (PLANE_SIZE - CUBE_SIZE) * 0.5);
        t.translation.x = t.translation.x.min((PLANE_SIZE - CUBE_SIZE) * 0.5);
        t.translation.z = t.translation.z.max(-1. * (PLANE_SIZE - CUBE_SIZE) * 0.5);
        t.translation.z = t.translation.z.min((PLANE_SIZE - CUBE_SIZE) * 0.5);
    }
}
