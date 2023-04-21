use crate::{world_snapshot::WorldSnapshot, GGRSSchedule, PlayerInputs, Session, Frame, GGRSSaveSchedule, GGRSLoadSchedule, GameStateCell};
use bevy::{prelude::*, reflect::TypeRegistry};
use ggrs::{
    Config, GGRSError, GGRSRequest, PlayerHandle, SessionState,
};
use instant::{Duration, Instant};

#[derive(Resource)]
/// The GGRSStage handles updating, saving and loading the game state.
pub(crate) struct GGRSStage<T>
where
    T: Config,
{
    /// Used to register all types considered when loading and saving
    pub(crate) type_registry: TypeRegistry,
    /// This system is used to get an encoded representation of the input that GGRS can handle
    pub(crate) input_system: Box<dyn System<In = PlayerHandle, Out = T::Input>>,
    /// Instead of using GGRS's internal storage for encoded save states, we save the world here, avoiding serialization into `Vec<u8>`.
    snapshots: Vec<WorldSnapshot>,
    /// fixed FPS our logic is running with
    update_frequency: usize,
    /// counts the number of frames that have been executed
    frame: i32,
    /// internal time control variables
    last_update: Instant,
    /// accumulated time. once enough time has been accumulated, an update is executed
    accumulator: Duration,
    /// boolean to see if we should run slow to let remote clients catch up
    run_slow: bool,
}

impl<T: Config + Send + Sync> GGRSStage<T> {
    pub(crate) fn run(world: &mut World) {
        let mut stage = world
            .remove_resource::<Self>()
            .expect("failed to extract ggrs schedule");

        // get delta time from last run() call and accumulate it
        let delta = Instant::now().duration_since(stage.last_update);
        let mut fps_delta = 1. / stage.update_frequency as f64;
        if stage.run_slow {
            fps_delta *= 1.1;
        }
        stage.accumulator = stage.accumulator.saturating_add(delta);
        stage.last_update = Instant::now();

        // no matter what, poll remotes and send responses
        if let Some(mut session) = world.get_resource_mut::<Session<T>>() {
            session.poll();
        }

        // if we accumulated enough time, do steps
        while stage.accumulator.as_secs_f64() > fps_delta {
            // decrease accumulator
            stage.accumulator = stage
                .accumulator
                .saturating_sub(Duration::from_secs_f64(fps_delta));

            // depending on the session type, doing a single update looks a bit different
            let Some(session) = world.get_resource::<Session<T>>() else {
                stage.reset();
                continue;
            };

            let requests = match session {
                &Session::SyncTestSession(_) => stage.run_synctest(world),
                &Session::P2PSession(_) => stage.run_p2p(world),
                &Session::SpectatorSession(_) => stage.run_spectator(world),
            };

            world.insert_resource(stage);
            for request in requests {
                match request {
                    GGRSRequest::SaveGameState { cell, frame } => {
                        debug!("saving snapshot for frame {frame}");

                        world.insert_resource(Frame(frame));
                        world.insert_resource(GameStateCell::<T>(cell));

                        world.run_schedule(GGRSSaveSchedule);

                        world.remove_resource::<GameStateCell::<T>>();
                    },

                    GGRSRequest::LoadGameState { cell, frame } => {
                        debug!("restoring snapshot for frame {frame}");

                        world.insert_resource(Frame(frame));
                        world.insert_resource(GameStateCell::<T>(cell));

                        world.run_schedule(GGRSLoadSchedule);

                        world.remove_resource::<GameStateCell::<T>>();
                    },

                    GGRSRequest::AdvanceFrame { inputs } => {
                        let mut frame = *world.get_resource::<Frame>().unwrap_or(&Frame(0));

                        debug!("advancing to frame: {}", frame.0 + 1);

                        world.insert_resource(PlayerInputs::<T>(inputs));

                        world.run_schedule(GGRSSchedule);

                        world.remove_resource::<PlayerInputs<T>>();

                        frame.0 += 1;
                        debug!("frame {} completed", frame.0);

                        world.insert_resource(frame);

                        world.resource_scope(|_world, mut stage: Mut<Self>| {
                            stage.frame = frame.0
                        });
                    },
                }
            }
            stage = world.remove_resource::<Self>().unwrap();
        }

        world.insert_resource(stage);
    }
}

impl<T: Config> GGRSStage<T> {
    pub(crate) fn new(input_system: Box<dyn System<In = PlayerHandle, Out = T::Input>>) -> Self {
        Self {
            type_registry: TypeRegistry::default(),
            input_system,
            snapshots: Vec::new(),
            frame: 0,
            update_frequency: 60,
            last_update: Instant::now(),
            accumulator: Duration::ZERO,
            run_slow: false,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.last_update = Instant::now();
        self.accumulator = Duration::ZERO;
        self.frame = 0;
        self.run_slow = false;
        self.snapshots = Vec::new();
    }

    pub(crate) fn run_synctest(&mut self, world: &mut World) -> Vec<GGRSRequest<T>> {
        // let ses = world.get_resource::<Session<T>>().expect("lol");
        let Some(Session::SyncTestSession(sess)) = world.get_resource::<Session<T>>() else {
            // TODO: improve error message for new API
            panic!("No GGRS SyncTestSession found. Please start a session and add it as a resource.");
        };

        // if our snapshot vector is not initialized, resize it accordingly
        if self.snapshots.is_empty() {
            for _ in 0..sess.max_prediction() {
                self.snapshots.push(WorldSnapshot::default());
            }
        }

        // get inputs for all players
        let mut inputs = Vec::new();
        for handle in 0..sess.num_players() {
            inputs.push(self.input_system.run(handle, world));
        }

        let mut sess = world.get_resource_mut::<Session<T>>();
        let Some(Session::SyncTestSession(ref mut sess)) = sess.as_deref_mut() else {
            panic!("No GGRS SyncTestSession found. Please start a session and add it as a resource.");
        };
        for (player_handle, &input) in inputs.iter().enumerate() {
            sess.add_local_input(player_handle, input)
                .expect("All handles between 0 and num_players should be valid");
        }

        match sess.advance_frame() {
            Ok(requests) => return requests,
            Err(e) => warn!("{}", e),
        }

        vec![]
    }

    pub(crate) fn run_spectator(&mut self, world: &mut World) -> Vec<GGRSRequest<T>> {
        // run spectator session, no input necessary
        let mut sess = world.get_resource_mut::<Session<T>>();
        let Some(Session::SpectatorSession(ref mut sess)) = sess.as_deref_mut() else {
            // TODO: improve error message for new API
            panic!("No GGRS P2PSpectatorSession found. Please start a session and add it as a resource.");
        };

        // if session is ready, try to advance the frame
        if sess.current_state() == SessionState::Running {
            match sess.advance_frame() {
                Ok(requests) => return requests,
                Err(GGRSError::PredictionThreshold) => {
                    info!("P2PSpectatorSession: Waiting for input from host.")
                }
                Err(e) => warn!("{}", e),
            };
        }

        vec![]
    }

    pub(crate) fn run_p2p(&mut self, world: &mut World) -> Vec<GGRSRequest<T>> {
        let sess = world.get_resource::<Session<T>>();
        let Some(Session::P2PSession(ref sess)) = sess else {
            // TODO: improve error message for new API
            panic!("No GGRS P2PSession found. Please start a session and add it as a resource.");
        };

        // if our snapshot vector is not initialized, resize it accordingly
        if self.snapshots.is_empty() {
            // find out what the maximum prediction window is in this synctest
            for _ in 0..sess.max_prediction() {
                self.snapshots.push(WorldSnapshot::default());
            }
        }

        // if we are ahead, run slow
        self.run_slow = sess.frames_ahead() > 0;

        // get local player handles
        let local_handles = sess.local_player_handles();

        // get local player inputs
        let mut local_inputs = Vec::new();
        for &local_handle in &local_handles {
            let input = self.input_system.run(local_handle, world);
            local_inputs.push(input);
        }

        // if session is ready, try to advance the frame
        let mut sess = world.get_resource_mut::<Session<T>>();
        let Some(Session::P2PSession(ref mut sess)) = sess.as_deref_mut() else {
            // TODO: improve error message for new API
            panic!("No GGRS P2PSession found. Please start a session and add it as a resource.");
        };
        if sess.current_state() == SessionState::Running {
            for i in 0..local_inputs.len() {
                sess.add_local_input(local_handles[i], local_inputs[i])
                    .expect("All handles in local_handles should be valid");
            }
            match sess.advance_frame() {
                Ok(requests) => return requests,
                Err(GGRSError::PredictionThreshold) => {
                    info!("Skipping a frame: PredictionThreshold.")
                }
                Err(e) => warn!("{}", e),
            };
        }

        vec![]
    }

    pub(crate) fn save_world_using_reflection(world: &mut World) {
        world.resource_scope(|world, mut stage: Mut<Self>| {
            let frame = world.get_resource::<Frame>().unwrap();
            let cell = world.get_resource::<GameStateCell<T>>().unwrap();

            assert_eq!(stage.frame, frame.0);

            // we make a snapshot of our world
            let snapshot = WorldSnapshot::from_world(world, &stage.type_registry);

            // we don't really use the buffer provided by GGRS
            cell.save(stage.frame, None, Some(snapshot.checksum as u128));

            // store the snapshot ourselves (since the snapshots don't implement clone)
            let pos = frame.0 as usize % stage.snapshots.len();
            stage.snapshots[pos] = snapshot;
        });
    }

    pub(crate) fn load_world_using_reflection(world: &mut World) {
        world.resource_scope(|world, mut stage: Mut<Self>| {
            let frame = world.get_resource::<Frame>().unwrap();

            stage.frame = frame.0;
    
            // we get the correct snapshot
            let pos = frame.0 as usize % stage.snapshots.len();
            let snapshot_to_load = &stage.snapshots[pos];
    
            // load the entities
            snapshot_to_load.write_to_world(world, &stage.type_registry);
        });
    }

    pub(crate) fn set_update_frequency(&mut self, update_frequency: usize) {
        self.update_frequency = update_frequency
    }

    pub(crate) fn set_type_registry(&mut self, type_registry: TypeRegistry) {
        self.type_registry = type_registry;
    }
}
