use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_system;
use game_engine::{
    core::state::GameState,
    domain::character::systems::ZoneSessionData,
    infrastructure::networking::{client::ZoneServerClient, protocol::zone::ChatReceived},
};
use tauri::Emitter;

use super::events::ChatRequestedEvent;
use crate::plugin::TauriSystems;

#[derive(Clone, serde::Serialize)]
struct ChatPayload {
    gid: u32,
    message: String,
}

#[auto_add_system(
    plugin = crate::plugin::TauriIntegrationAutoPlugin,
    schedule = Update,
    config(in_set = TauriSystems::Handlers, run_if = in_state(GameState::InGame))
)]
pub fn handle_chat_request(
    mut events: MessageReader<ChatRequestedEvent>,
    mut client: Option<ResMut<ZoneServerClient>>,
    zone_session: Option<Res<ZoneSessionData>>,
) {
    for event in events.read() {
        let Some(client) = client.as_deref_mut() else {
            warn!("Cannot send chat message: ZoneServerClient not available");
            continue;
        };

        let Some(session) = zone_session.as_ref() else {
            warn!("Cannot send chat message: ZoneSessionData not available");
            continue;
        };

        let formatted_message = format!("{} : {}", session.character_name, event.message);
        if let Err(e) = client.send_chat_message(formatted_message) {
            error!("Failed to send chat message: {:?}", e);
        }
    }
}

#[auto_add_system(
    plugin = crate::plugin::TauriIntegrationAutoPlugin,
    schedule = Update,
    config(in_set = TauriSystems::Emitters, run_if = in_state(GameState::InGame))
)]
pub fn emit_chat_events(
    mut events: MessageReader<ChatReceived>,
    app_handle: NonSend<tauri::AppHandle>,
) {
    for event in events.read() {
        let payload = ChatPayload {
            gid: event.gid,
            message: event.message.clone(),
        };

        if let Err(e) = app_handle.emit("chat-message-received", payload) {
            error!("Failed to emit chat-message-received event: {:?}", e);
        }
    }
}
