use crate::infrastructure::networking::{
    ConnectionState, errors::NetworkResult, protocols::ro_login::AcAcceptLoginPacket,
};
use bevy::prelude::*;
use tokio::task::JoinHandle;

#[derive(Component)]
pub struct LoginTask {
    pub username: String,
    pub task: JoinHandle<NetworkResult<AcAcceptLoginPacket>>,
}

#[derive(Component, Debug)]
pub struct AuthenticationAttempt {
    pub username: String,
    pub started_at: std::time::Instant,
}

#[derive(Component, Debug)]
pub struct ConnectionStateComponent {
    pub state: ConnectionState,
}

impl Default for ConnectionStateComponent {
    fn default() -> Self {
        Self {
            state: ConnectionState::Disconnected,
        }
    }
}
