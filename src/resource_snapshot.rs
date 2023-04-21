use std::{collections::VecDeque, marker::PhantomData};

use bevy::prelude::{Resource, Commands, ResMut, Res, Plugin, IntoSystemAppConfig};
use ggrs::Config;
use log::warn;

use crate::{Frame, Session, GGRSSaveSchedule, GGRSLoadSchedule};

/// Plugin adding a `Clone`-based rollback mechanism for a particular resource.
pub struct ResourceRollbackPlugin;

impl ResourceRollbackPlugin {
    pub fn for_type<R: Resource>(
        self,
    ) -> ResourceRollbackPluginBuilder<(), R> {
        ResourceRollbackPluginBuilder(Default::default())
    }
}

pub struct ResourceRollbackPluginBuilder<T, R>(PhantomData<(T, R)>);

impl<T, R> ResourceRollbackPluginBuilder<T, R> {
    pub fn with_config<TNew>(self) -> ResourceRollbackPluginBuilder<TNew, R>
    where
        TNew: Config,
    {
        ResourceRollbackPluginBuilder(Default::default())
    }
}

impl<T, R> Plugin for ResourceRollbackPluginBuilder<T, R>
where
    Self: 'static + Send + Sync,
    T: Config,
    R: Resource + Clone,
{
    fn build(&self, app: &mut bevy::prelude::App) {
        app.init_resource::<ResourceHistory<R>>()
            .add_system(ResourceHistory::<R>::save::<T>.in_schedule(GGRSSaveSchedule))
            .add_system(ResourceHistory::<R>::load::<T>.in_schedule(GGRSLoadSchedule));
    }
}

/// Resource for storing the state of a resource type over time.
#[derive(Resource)]
pub struct ResourceHistory<R>
where
    R: Resource,
{
    states: VecDeque<(Frame, Option<R>)>,
}

impl<R> Default for ResourceHistory<R>
where
    R: Resource,
{
    fn default() -> Self {
        Self {
            states: Default::default(),
        }
    }
}

impl<R> ResourceHistory<R>
where
    Self: 'static,
    R: Send + Sync + Resource + Clone,
{
    /// Checks the `Frame` and `ResourceHistory` resources to rollback a resource `R`
    pub fn load<T>(
        mut commands: Commands,
        resource: Option<ResMut<R>>,
        history: Res<Self>,
        frame: Res<Frame>,
    ) where
        T: Config,
    {
        // Find the snapshot for our desired frame.
        let Some((_, snapshot)) = history
            .states
            .iter()
            .find(|(stored_frame, _)| *stored_frame == *frame) else {
                warn!("Cannot load frame: {}", frame.0);
                return;
            };

        // Restore resource state
        match (resource, snapshot) {
            (None, None) => {
                // No action required
            }
            (None, Some(snapshot)) => {
                commands.insert_resource(snapshot.clone());
            }
            (Some(_), None) => {
                commands.remove_resource::<R>();
            }
            (Some(mut resource), Some(snapshot)) => {
                *resource = snapshot.clone();
            }
        }
    }

    /// Saves the state of the resource `R` to the appropriate `ResourceHistory`
    pub fn save<T>(
        resource: Option<Res<R>>,
        mut history: ResMut<Self>,
        frame: Res<Frame>,
        session: Res<Session<T>>,
    ) where
        T: Config,
    {
        let snapshot = resource.map(|resource| resource.clone());

        history.states.push_front((*frame, snapshot));

        while history.states.len() > session.max_prediction_window() {
            history.states.pop_back();
        }
    }
}