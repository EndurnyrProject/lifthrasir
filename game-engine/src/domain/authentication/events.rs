use crate::infrastructure::networking::{errors::NetworkError, session::UserSession};
use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_event;

#[derive(Event, Debug)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginAttemptStartedEvent {
    pub username: String,
}

#[derive(Event, Debug, Clone)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginSuccessEvent {
    pub session: UserSession,
}

#[derive(Event, Debug)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginFailureEvent {
    pub error: NetworkError,
    pub username: String,
}

#[derive(Event, Debug)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LogoutEvent;

#[derive(Event, Debug)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct SessionExpiredEvent;
