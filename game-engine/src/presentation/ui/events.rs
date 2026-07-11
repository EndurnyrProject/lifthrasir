use crate::core::state::GameState;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_message, auto_register_type};
use net_contract::dto::ServerInfo;
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

/// Who raised the dialog, so a [`SystemDialogChoice`] can be routed back to the right
/// consumer even when two dialogs contend in one tick. Carried on both the request and
/// the emitted choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum SystemDialogKind {
    #[default]
    Generic,
    PartyInvite,
}

/// Summons the reusable system-dialog modal (see `lifthrasir-ui` `SystemDialog`).
/// `code` empty hides the error-code chip; `secondary_label` empty hides the
/// secondary button; `confirm_state` is the screen the primary button navigates
/// to (`None` simply dismisses).
#[derive(Message, Clone)]
#[auto_add_message(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct ShowSystemDialog {
    pub severity: DialogSeverity,
    pub kind: SystemDialogKind,
    pub kicker: String,
    pub title: String,
    pub message: String,
    pub code: String,
    pub button_label: String,
    pub secondary_label: String,
    pub confirm_state: Option<GameState>,
}

/// Emitted by the system dialog when either button is pressed: `primary` is true
/// for the primary (confirm) button, false for the secondary (dismiss) button.
/// `kind` echoes the raising [`ShowSystemDialog`] so a consumer claims only its own
/// dialog's choice, immune to two dialogs contending in one tick.
#[derive(Message, Clone)]
#[auto_add_message(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct SystemDialogChoice {
    pub primary: bool,
    pub kind: SystemDialogKind,
}
