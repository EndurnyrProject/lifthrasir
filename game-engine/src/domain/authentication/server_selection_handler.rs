use crate::{
    core::state::GameState,
    infrastructure::networking::{session::UserSession, CharServerClient},
    presentation::ui::events::ServerSelectedEvent,
};
use bevy::prelude::*;

/// System that handles server selection events
/// Updates the session and connects to character server (UI flow handled by Tauri)
pub fn handle_server_selection(
    mut commands: Commands,
    mut server_events: EventReader<ServerSelectedEvent>,
    mut session: ResMut<UserSession>,
    mut char_client: Option<ResMut<CharServerClient>>,
) {
    for event in server_events.read() {
        info!("Server selected: {}", event.server.name);

        // Update session with selected server
        session.selected_server = Some(event.server.clone());

        // Create or reconnect CharServerClient
        if let Some(client) = char_client.as_deref_mut() {
            // Disconnect existing connection if any
            client.disconnect();

            // Connect to the selected server
            let ip_bytes = event.server.ip.to_be_bytes();
            let server_ip = format!(
                "{}.{}.{}.{}",
                ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]
            );

            if let Err(e) = client.connect(&server_ip, event.server.port) {
                error!("Failed to connect to character server: {:?}", e);
            } else {
                info!(
                    "Connected to character server at {}:{}",
                    server_ip, event.server.port
                );
            }
        } else {
            // Create new CharServerClient
            let mut client = CharServerClient::new(session.clone());

            let ip_bytes = event.server.ip.to_be_bytes();
            let server_ip = format!(
                "{}.{}.{}.{}",
                ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]
            );

            if let Err(e) = client.connect(&server_ip, event.server.port) {
                error!("Failed to connect to character server: {:?}", e);
            } else {
                info!(
                    "Connected to character server at {}:{}",
                    server_ip, event.server.port
                );
            }

            commands.insert_resource(client);
        }

        // Note: State transition to character selection is now handled by Tauri UI
        // The Bevy backend stays in ServerSelection state while Tauri shows character selection UI
    }
}
