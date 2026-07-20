//! In-game chat send path.
//!
//! The UI writes [`ChatSendRequested`] when the player submits a chat line; this
//! handler formats it as `"<character name> : <message>"` (the format the zone
//! server expects) and emits a [`ChatSent`] contract command. The net-aesir
//! `send::social` system turns that into a `ChatRequest` on the QUIC GAMEPLAY
//! channel. Incoming chat arrives separately as `ChatHeard` (read by the UI).
//!
//! This was previously the Tauri bridge's `handle_chat_request`; it now lives in
//! the engine so the native UI only has to emit a plain event.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use net_contract::commands::ChatSent;

use crate::core::state::GameState;
use crate::domain::entities::components::EntityName;
use crate::domain::entities::markers::LocalPlayer;

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
    mut chat_requests: MessageWriter<ChatSent>,
    player: Query<&EntityName, With<LocalPlayer>>,
) {
    for event in events.read() {
        if event.message.trim().is_empty() {
            continue;
        }
        let Ok(player) = player.single() else {
            warn!("Cannot send chat message: local player name not available");
            continue;
        };
        let formatted = format_chat_message(&player.name, &event.message);
        chat_requests.write(ChatSent { message: formatted });
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
