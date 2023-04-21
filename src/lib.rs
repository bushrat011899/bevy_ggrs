//! bevy_ggrs is a bevy plugin for the P2P rollback networking library GGRS.
#![forbid(unsafe_code)] // let us try

use bevy::{
    ecs::schedule::{LogLevel, ScheduleBuildSettings, ScheduleLabel},
    prelude::*,
    reflect::{FromType, GetTypeRegistration, TypeRegistry, TypeRegistryInternal},
};
use ggrs::{Config, InputStatus, PlayerHandle};
use ggrs_stage::GGRSStage;
use parking_lot::RwLock;
use std::sync::Arc;

pub use ggrs;

pub use rollback::{Rollback, RollbackFlag};
pub use session::Session;
pub use resource_snapshot::ResourceRollbackPlugin;

pub(crate) mod ggrs_stage;
pub(crate) mod world_snapshot;
pub(crate) mod rollback;
pub(crate) mod session;
pub(crate) mod resource_snapshot;

const DEFAULT_FPS: usize = 60;

/// Schedule run during the frame-advancement stage of GGRS rollback networking.
#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
pub struct GGRSSchedule;

/// Schedule run during the saving stage of GGRS rollback networking.
#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
pub struct GGRSSaveSchedule;

/// Schedule run during the loading stage of GGRS rollback networking.
#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
pub struct GGRSLoadSchedule;

// TODO: more specific name to avoid conflicts?
#[derive(Resource, Deref, DerefMut)]
pub struct PlayerInputs<T: Config>(Vec<(T::Input, InputStatus)>);

#[derive(Resource, Deref, DerefMut, Clone, Copy, PartialEq, Eq, Default)]
pub struct Frame(ggrs::Frame);

impl Frame {
    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

#[derive(Resource, Deref, DerefMut)]
pub struct GameStateCell<T: Config>(ggrs::GameStateCell<T::State>);

/// A builder to configure GGRS for a bevy app.
pub struct GGRSPlugin<T: Config + Send + Sync> {
    input_system: Option<Box<dyn System<In = PlayerHandle, Out = T::Input>>>,
    fps: usize,
    type_registry: TypeRegistry,
}

impl<T: Config + Send + Sync> Default for GGRSPlugin<T> {
    fn default() -> Self {
        Self {
            input_system: None,
            fps: DEFAULT_FPS,
            type_registry: TypeRegistry {
                internal: Arc::new(RwLock::new({
                    let mut r = TypeRegistryInternal::empty();
                    // `Parent` and `Children` must be registered so that their `ReflectMapEntities`
                    // data may be used.
                    //
                    // While this is a little bit of a weird spot to register these, are the only
                    // Bevy core types implementing `MapEntities`, so for now it's probably fine to
                    // just manually register these here.
                    //
                    // The user can still register any custom types with `register_rollback_type()`.
                    r.register::<Parent>();
                    r.register::<Children>();
                    r
                })),
            },
        }
    }
}

impl<T: Config + Send + Sync> GGRSPlugin<T> {
    /// Create a new instance of the builder.
    pub fn new() -> Self {
        Default::default()
    }

    /// Change the update frequency of the rollback stage.
    pub fn with_update_frequency(mut self, fps: usize) -> Self {
        self.fps = fps;
        self
    }

    /// Registers a system that takes player handles as input and returns the associated inputs for that player.
    pub fn with_input_system<Params>(
        mut self,
        input_fn: impl IntoSystem<PlayerHandle, T::Input, Params>,
    ) -> Self {
        self.input_system = Some(Box::new(IntoSystem::into_system(input_fn)));
        self
    }

    /// Registers a type of component for saving and loading during rollbacks.
    pub fn register_rollback_component<Type>(self) -> Self
    where
        Type: GetTypeRegistration + Reflect + Default + Component,
    {
        let mut registry = self.type_registry.write();
        registry.register::<Type>();

        let registration = registry.get_mut(std::any::TypeId::of::<Type>()).unwrap();
        registration.insert(<ReflectComponent as FromType<Type>>::from_type());
        drop(registry);
        self
    }

    /// Registers a type of resource for saving and loading during rollbacks.
    pub fn register_rollback_resource<Type>(self) -> Self
    where
        Type: GetTypeRegistration + Reflect + Default + Resource,
    {
        let mut registry = self.type_registry.write();
        registry.register::<Type>();

        let registration = registry.get_mut(std::any::TypeId::of::<Type>()).unwrap();
        registration.insert(<ReflectResource as FromType<Type>>::from_type());
        drop(registry);
        self
    }

    /// Consumes the builder and makes changes on the bevy app according to the settings.
    pub fn build(self, app: &mut App) {
        let mut input_system = self
            .input_system
            .expect("Adding an input system through GGRSBuilder::with_input_system is required");

        // ggrs stage
        input_system.initialize(&mut app.world);
        let mut stage = GGRSStage::<T>::new(input_system);
        stage.set_update_frequency(self.fps);
        stage.set_type_registry(self.type_registry);
        app.insert_resource(stage);

        let mut advance_schedule = Schedule::default();
        let mut save_schedule = Schedule::default();
        let mut load_schedule = Schedule::default();

        advance_schedule.set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Error,
            ..default()
        });

        save_schedule.set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Warn,
            ..default()
        });

        load_schedule.set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Warn,
            ..default()
        });

        app.add_schedule(GGRSSchedule, advance_schedule);
        app.add_schedule(GGRSSaveSchedule, save_schedule);
        app.add_schedule(GGRSLoadSchedule, load_schedule);

        app.add_system(GGRSStage::<T>::run.in_base_set(CoreSet::PreUpdate));
        app.add_system(
            GGRSStage::<T>::save_world_using_reflection
                .in_schedule(GGRSSaveSchedule)
                .in_base_set(CoreSet::First)
        );
        app.add_system(
            GGRSStage::<T>::load_world_using_reflection
                .in_schedule(GGRSLoadSchedule)
                .in_base_set(CoreSet::First)
            );
    }
}
