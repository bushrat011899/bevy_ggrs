use bevy::ecs::{
    event::EventReader,
    prelude::Resource,
    schedule::ScheduleLabel,
    system::SystemState,
    world::World,
};
use bevy::prelude::Mut;
use ggrs::{Config, GGRSRequest};
use log::debug;

use crate::{Frame, GameStateCell, PlayerInputs};

/// Schedule run during the frame-advancement stage of GGRS rollback networking.
#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
pub struct GGRSSchedule;

/// Schedule run during the saving stage of GGRS rollback networking.
#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
pub struct GGRSSaveSchedule;

/// Schedule run during the loading stage of GGRS rollback networking.
#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
pub struct GGRSLoadSchedule;

#[derive(Resource)]
struct RunStateSchedulesCache<T: Config> {
    event_state: SystemState<EventReader<'static, 'static, GGRSRequest<T>>>,
}

enum Request<T: Config> {
    Save(Frame, GameStateCell<T>),
    Load(Frame, GameStateCell<T>),
    Advance(PlayerInputs<T>),
}

impl<S: Config> From<&GGRSRequest<S>> for Request<S> {
    fn from(value: &GGRSRequest<S>) -> Self {
        match value {
            GGRSRequest::SaveGameState { cell, frame } => {
                Request::Save(Frame(*frame), GameStateCell(cell.clone()))
            }
            GGRSRequest::LoadGameState { cell, frame } => {
                Request::Load(Frame(*frame), GameStateCell(cell.clone()))
            }
            GGRSRequest::AdvanceFrame { inputs } => Request::Advance(PlayerInputs(inputs.clone())),
        }
    }
}

/// System to marshal state action events into the Bevy Schedule functionality.
///
/// The role of this system is to execute the `SaveSchedule`, `LoadSchedule`, and
/// `AdvanceSchedule` in response to the appropriate `GGRSRequest` events. In addition,
/// it is also responsible for inserting the `DesiredState` resource during the save and
/// load schedules, and the `Inputs` resource during frame advancement.
///
/// Because `GGRSRequest` events are collected prior to running the relevant schedules, any
/// `GGRSRequest` events published during those schedules won't be actioned until the next
/// round of execution (i.e., a one frame delay)
pub fn run_schedules<T>(world: &mut World)
where
    T: Config,
{
    if !world.contains_resource::<RunStateSchedulesCache<T>>() {
        let cache = RunStateSchedulesCache {
            event_state: SystemState::<EventReader<GGRSRequest<T>>>::new(world),
        };
        world.insert_resource(cache);
    }

    world.resource_scope(|world, mut cache: Mut<RunStateSchedulesCache<T>>| {
        cache
            .event_state
            .get_mut(world)
            .into_iter()
            .map(|request| request.into())
            .collect::<Vec<Request<T>>>()
            .into_iter()
            .for_each(|action| match action {
                Request::Save(frame, state) => {
                    debug!("saving snapshot for frame {}", frame.0);
    
                    world.insert_resource(frame);
                    world.insert_resource(state);
    
                    world.run_schedule(GGRSSaveSchedule);
    
                    world.remove_resource::<GameStateCell::<T>>();
                }

                Request::Load(frame, state) => {
                    debug!("restoring snapshot for frame {}", frame.0);
    
                    world.insert_resource(frame);
                    world.insert_resource(state);
    
                    world.run_schedule(GGRSLoadSchedule);
    
                    world.remove_resource::<GameStateCell::<T>>();
                }

                Request::Advance(inputs) => {
                    let next_frame = world
                        .get_resource::<Frame>()
                        .copied()
                        .unwrap_or_default()
                        .next();

                    debug!("advancing to frame: {}", next_frame.0);

                    world.insert_resource(inputs);

                    world.run_schedule(GGRSSchedule);

                    world.remove_resource::<PlayerInputs<T>>();

                    debug!("frame {} completed", next_frame.0);

                    world.insert_resource(next_frame);
                }
            });
    });
}