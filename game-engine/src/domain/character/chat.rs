//! In-game chat send path.
//!
//! The UI writes [`ChatSendRequested`] when the player submits a chat line; this
//! handler formats it as `"<character name> : <message>"` (the format the zone
//! server expects) and ships it as a `ChatRequest` over the QUIC GAMEPLAY channel.
//! Incoming chat arrives separately as `ChatHeard` (read directly by the UI).
//!
//! This was previously the Tauri bridge's `handle_chat_request`; it now lives in
//! the engine so the native UI only has to emit a plain event.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_quinnet::client::QuinnetClient;

use crate::core::state::GameState;
use crate::domain::character::systems::ZoneSessionData;
use crate::infrastructure::networking::quic::channels::GAMEPLAY;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::quic::proto::aesir::net::ChatRequest;
use crate::infrastructure::networking::quic::zone::{QuicZoneState, ZonePhase};

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
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
    zone_session: Option<Res<ZoneSessionData>>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }

    for event in events.read() {
        if event.message.trim().is_empty() {
            continue;
        }
        let Some(session) = zone_session.as_ref() else {
            warn!("Cannot send chat message: ZoneSessionData not available");
            continue;
        };
        let formatted = format_chat_message(&session.character_name, &event.message);
        let body = Body::ChatRequest(ChatRequest { message: formatted });
        if let Err(e) = zone.send(&mut client, GAMEPLAY, body) {
            error!("Failed to send chat message: {e}");
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
