use crate::infrastructure::networking::protocol::login::types::ServerInfo;
use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::{auto_add_event, auto_register_type};
use secrecy::SecretString;

#[derive(Message, Clone, Reflect)]
#[reflect(opaque)]
#[auto_register_type(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginAttemptEvent {
    pub username: String,
    #[reflect(ignore)]
    pub password: SecretString,
}

#[derive(Message, Clone)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct ServerSelectedEvent {
    pub server: ServerInfo,
    /// Optional server index for correlation with UI requests
    pub server_index: Option<usize>,
}

#[derive(Message)]
pub struct CharacterSelectEvent {
    pub character_id: u32,
}

#[derive(Message)]
pub struct CreateCharacterEvent;

#[derive(Message)]
pub struct DeleteCharacterEvent {
    pub character_id: u32,
}

#[derive(Message)]
pub struct BackToLoginEvent;

#[derive(Message)]
pub struct BackToServerSelectionEvent;
