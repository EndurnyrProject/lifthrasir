use crate::infrastructure::networking::{errors::NetworkError, session::UserSession};
use bevy::prelude::*;

#[derive(Event, Debug)]
pub struct LoginAttemptStartedEvent {
    pub username: String,
}

#[derive(Event, Debug, Clone)]
pub struct LoginSuccessEvent {
    pub session: UserSession,
}

#[derive(Event, Debug)]
pub struct LoginFailureEvent {
    pub error: NetworkError,
    pub username: String,
}

#[derive(Event, Debug)]
pub struct LogoutEvent;

#[derive(Event, Debug)]
pub struct SessionExpiredEvent;
