use crate::infrastructure::networking::{errors::NetworkError, session::UserSession};
use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_event;

#[derive(Message, Debug)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginAttemptStartedEvent {
    pub username: String,
}

#[derive(Message, Debug, Clone)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginSuccessEvent {
    pub session: UserSession,
}

#[derive(Message, Debug)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginFailureEvent {
    pub error: NetworkError,
    pub username: String,
}

#[derive(Message, Debug)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LogoutEvent;

#[derive(Message, Debug)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct SessionExpiredEvent;
