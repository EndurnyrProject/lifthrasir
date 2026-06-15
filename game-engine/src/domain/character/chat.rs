//! In-game chat send path.
//!
//! The UI writes [`ChatSendRequested`] when the player submits a chat line; this
//! handler formats it as `"<character name> : <message>"` (the format rAthena's
//! zone server expects) and ships it through [`ZoneServerClient`]. Incoming chat
//! arrives separately as `ChatReceived` (read directly by the UI).
//!
//! This was previously the Tauri bridge's `handle_chat_request`; it now lives in
//! the engine so the native UI only has to emit a plain event.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::core::state::GameState;
use crate::domain::character::systems::ZoneSessionData;
use crate::infrastructure::networking::client::ZoneServerClient;

/// Emitted by the UI when the player submits a chat line.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct ChatSendRequested {
    pub message: String,
}

/// Formats a chat line the way the zone server expects: `"<name> : <message>"`.
pub fn format_chat_message(character_name: &str, message: &str) -> String {
    format!("{character_name} : {message}")
}

#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn handle_chat_send(
    mut events: MessageReader<ChatSendRequested>,
    mut client: Option<ResMut<ZoneServerClient>>,
    zone_session: Option<Res<ZoneSessionData>>,
) {
    for event in events.read() {
        if event.message.trim().is_empty() {
            continue;
        }
        let Some(client) = client.as_deref_mut() else {
            warn!("Cannot send chat message: ZoneServerClient not available");
            continue;
        };
        let Some(session) = zone_session.as_ref() else {
            warn!("Cannot send chat message: ZoneSessionData not available");
            continue;
        };
        let formatted = format_chat_message(&session.character_name, &event.message);
        if let Err(e) = client.send_chat_message(formatted) {
            error!("Failed to send chat message: {:?}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_name_and_message() {
        assert_eq!(format_chat_message("Hero", "hello"), "Hero : hello");
    }

    #[test]
    fn preserves_message_spacing() {
        assert_eq!(
            format_chat_message("Valkyrie", "  spaced  out  "),
            "Valkyrie :   spaced  out  "
        );
    }
}
