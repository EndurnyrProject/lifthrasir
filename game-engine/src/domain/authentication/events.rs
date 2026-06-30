use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;
use net_contract::{dto::NetworkError, state::UserSession};

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginAttemptStartedEvent {
    pub username: String,
}

#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginSuccessEvent {
    pub session: UserSession,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginFailureEvent {
    pub error: NetworkError,
    pub username: String,
}
