use bevy::prelude::Resource;
use ggrs::{Config, P2PSession, SpectatorSession, SyncTestSession, GGRSRequest, GGRSError, PlayerHandle, GGRSEvent};

/// Defines the Session that the GGRS Plugin should expect as a resource.
#[derive(Resource)]
pub enum Session<T: Config> {
    SyncTestSession(SyncTestSession<T>),
    P2PSession(P2PSession<T>),
    SpectatorSession(SpectatorSession<T>),
}

impl<T: Config> From<P2PSession<T>> for Session<T> {
    fn from(value: P2PSession<T>) -> Self {
        Self::P2PSession(value)
    }
}

impl<T: Config> From<SyncTestSession<T>> for Session<T> {
    fn from(value: SyncTestSession<T>) -> Self {
        Self::SyncTestSession(value)
    }
}

impl<T: Config> From<SpectatorSession<T>> for Session<T> {
    fn from(value: SpectatorSession<T>) -> Self {
        Self::SpectatorSession(value)
    }
}

impl<T: Config> Session<T> {
    /// Triggers the appropriate polling function depending on the exact variant of the `Session`.
    pub fn poll(&mut self) {
        match self {
            Self::P2PSession(session) => session.poll_remote_clients(),
            Self::SpectatorSession(session) => session.poll_remote_clients(),
            Self::SyncTestSession(_) => {}
        }
    }

    /// Try to advance the state of the session by one frame.
    pub fn try_advance_frame(&mut self) -> Result<Vec<GGRSRequest<T>>, GGRSError> {
        match self {
            Self::P2PSession(session) => session.advance_frame(),
            Self::SpectatorSession(session) => session.advance_frame(),
            Self::SyncTestSession(session) => session.advance_frame(),
        }
    }

    /// Try to set the inputs for a particular local player.
    pub fn try_set_input(
        &mut self,
        handle: PlayerHandle,
        input: T::Input,
    ) -> Result<(), GGRSError> {
        match self {
            Self::P2PSession(session) => session.add_local_input(handle, input),
            Self::SpectatorSession(_session) => Ok(()),
            Self::SyncTestSession(session) => session.add_local_input(handle, input),
        }
    }

    /// Drain available events.
    pub fn events(&mut self) -> Vec<GGRSEvent<T>> {
        match self {
            Self::SyncTestSession(_session) => vec![],
            Self::P2PSession(session) => session.events().collect(),
            Self::SpectatorSession(session) => session.events().collect(),
        }
    }

    /// Estimate the number of frames this client is ahead of the other clients.
    pub fn frames_ahead(&self) -> i32 {
        match self {
            Self::SyncTestSession(_session) => 0,
            Self::P2PSession(session) => session.frames_ahead(),
            Self::SpectatorSession(session) => -(session.frames_behind_host() as i32),
        }
    }

    /// Get the maximum prediction window.
    pub fn max_prediction_window(&self) -> usize {
        match self {
            Self::SyncTestSession(session) => session.max_prediction(),
            Self::P2PSession(session) => session.max_prediction(),
            Self::SpectatorSession(_session) => 0,
        }
    }
}