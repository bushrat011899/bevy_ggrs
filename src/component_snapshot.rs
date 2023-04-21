use std::{collections::VecDeque, marker::PhantomData};

use bevy::{prelude::{Resource, Commands, ResMut, Res, Plugin, IntoSystemAppConfig, Component, Query, Entity}, utils::HashMap};
use ggrs::Config;

use crate::{Frame, Session, GGRSSaveSchedule, GGRSLoadSchedule, Rollback};

/// Plugin adding a `Clone`-based rollback mechanism for a particular resource.
pub struct ComponentRollbackPlugin;

impl ComponentRollbackPlugin {
    pub fn for_type<C: Component>(
        self,
    ) -> ComponentRollbackPluginBuilder<(), C> {
        ComponentRollbackPluginBuilder(Default::default())
    }
}

pub struct ComponentRollbackPluginBuilder<T, C>(PhantomData<(T, C)>);

impl<T, C> ComponentRollbackPluginBuilder<T, C> {
    pub fn with_config<TNew>(self) -> ComponentRollbackPluginBuilder<TNew, C>
    where
        TNew: Config,
    {
        ComponentRollbackPluginBuilder(Default::default())
    }
}

impl<T, C> Plugin for ComponentRollbackPluginBuilder<T, C>
where
    Self: 'static + Send + Sync,
    T: Config,
    C: Component + Clone,
{
    fn build(&self, app: &mut bevy::prelude::App) {
        app.init_resource::<ComponentHistory<C>>()
            .add_system(ComponentHistory::<C>::save::<T>.in_schedule(GGRSSaveSchedule))
            .add_system(ComponentHistory::<C>::load::<T>.in_schedule(GGRSLoadSchedule));
    }
}

/// Resource for storing the state of a resource type over time.
#[derive(Resource)]
pub struct ComponentHistory<C>
where
    C: Component,
{
    states: VecDeque<(Frame, HashMap<Rollback, C>)>,
}

impl<C> Default for ComponentHistory<C>
where
    C: Component,
{
    fn default() -> Self {
        Self {
            states: Default::default(),
        }
    }
}

impl<C> ComponentHistory<C>
where
    Self: 'static,
    C: Send + Sync + Component + Clone,
{
    /// Checks the `Frame` and `ComponentHistory` resources to rollback all components of type `C`
    pub fn load<T>(
        mut commands: Commands,
        mut query: Query<(Entity, &Rollback, Option<&mut C>)>,
        history: Res<Self>,
        frame: Res<Frame>,
    ) where
        T: Config,
    {
        // Find the snapshot for our desired frame.
        let (_loaded_frame, map) = history
            .states
            .iter()
            .find(|(loaded_frame, _)| *loaded_frame == *frame)
            .expect(format!("Cannot load frame: {}", frame.0).as_str());

        let mut done = 0;

        // Restore component state
        for (entity, rollback, component) in query.iter_mut() {
            match (component, map.get(rollback)) {
                (None, None) => {
                    // No action required
                }
                (None, Some(snapshot)) => {
                    commands.entity(entity).insert(snapshot.clone());
                    done += 1;
                }
                (Some(_), None) => {
                    commands.entity(entity).remove::<C>();
                }
                (Some(mut component), Some(snapshot)) => {
                    *component = snapshot.clone();
                    done += 1;
                }
            }
        }

        assert!(
            done == map.len(),
            "Not all components could be added back into the world."
        );
    }

    /// Saves the state of the resource `R` to the appropriate `ResourceHistory`
    pub fn save<T>(
        query: Query<(&C, &Rollback)>,
        mut history: ResMut<Self>,
        frame: Res<Frame>,
        session: Res<Session<T>>,
    ) where
        T: Config,
    {
        let mut map = HashMap::new();

        for (component, flag) in query.iter() {
            map.insert(*flag, component.clone());
        }

        history.states.push_front((*frame, map));

        while history.states.len() > session.max_prediction_window() {
            history.states.pop_back();
        }
    }
}