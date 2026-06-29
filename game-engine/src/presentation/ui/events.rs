use crate::core::state::GameState;
use crate::infrastructure::networking::server_info::ServerInfo;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_message, auto_register_type};
use secrecy::SecretString;

#[derive(Message, Clone, Reflect)]
#[reflect(opaque)]
#[auto_register_type(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
#[auto_add_message(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginAttemptEvent {
    pub username: String,
    #[reflect(ignore)]
    pub password: SecretString,
}

#[derive(Message, Clone)]
#[auto_add_message(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct ServerSelectedEvent {
    pub server: ServerInfo,
}

/// Visual tone of a [`ShowSystemDialog`], driving the accent colour and badge glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum DialogSeverity {
    Error,
    Warn,
    Info,
    Ok,
}

/// Summons the reusable system-dialog modal (see `lifthrasir-ui` `SystemDialog`).
/// `code` empty hides the error-code chip; `confirm_state` is the screen the OK
/// button navigates to (`None` simply dismisses).
#[derive(Message, Clone)]
#[auto_add_message(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct ShowSystemDialog {
    pub severity: DialogSeverity,
    pub kicker: String,
    pub title: String,
    pub message: String,
    pub code: String,
    pub button_label: String,
    pub confirm_state: Option<GameState>,
}
