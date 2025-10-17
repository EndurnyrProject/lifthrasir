use crate::infrastructure::networking::protocol::login::types::ServerInfo;
use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::{auto_add_event, auto_register_type};
use secrecy::SecretString;

#[derive(Event, Clone, Reflect)]
#[reflect(opaque)]
#[auto_register_type(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginAttemptEvent {
    pub username: String,
    #[reflect(ignore)]
    pub password: SecretString,
}

#[derive(Event, Clone)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct ServerSelectedEvent {
    pub server: ServerInfo,
}

#[derive(Event)]
pub struct CharacterSelectEvent {
    pub character_id: u32,
}

#[derive(Event)]
pub struct CreateCharacterEvent;

#[derive(Event)]
pub struct DeleteCharacterEvent {
    pub character_id: u32,
}

#[derive(Event)]
pub struct BackToLoginEvent;

#[derive(Event)]
pub struct BackToServerSelectionEvent;
