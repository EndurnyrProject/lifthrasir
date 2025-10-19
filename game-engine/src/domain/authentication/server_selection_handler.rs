use crate::{
    infrastructure::networking::{client::CharServerClient, session::UserSession},
    presentation::ui::events::ServerSelectedEvent,
};
use bevy::prelude::*;

/// System that handles server selection events
/// Updates the session and connects to character server (UI flow handled by Tauri)
pub fn handle_server_selection(
    mut commands: Commands,
    mut server_events: MessageReader<ServerSelectedEvent>,
    mut session: ResMut<UserSession>,
    mut char_client: Option<ResMut<CharServerClient>>,
) {
    for event in server_events.read() {
        info!("Server selected: {}", event.server.name);

        session.selected_server = Some(event.server.clone());

        if let Some(client) = char_client.as_deref_mut() {
            client.disconnect();

            // Connect to the selected server
            let ip_bytes = event.server.ip.to_be_bytes();
            let server_ip = format!(
                "{}.{}.{}.{}",
                ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]
            );
            let address = format!("{}:{}", server_ip, event.server.port);

            if let Err(e) = client.connect(&address) {
                error!("Failed to connect to character server: {:?}", e);
            } else {
                info!("Connected to character server at {}", address);
                // Send CH_ENTER packet immediately after connection
                if let Err(e) = client.enter_server() {
                    error!("Failed to send CH_ENTER: {:?}", e);
                }
            }
        } else {
            let mut client = CharServerClient::with_session(
                session.tokens.account_id,
                session.tokens.login_id1,
                session.tokens.login_id2,
                session.sex,
            );

            let ip_bytes = event.server.ip.to_be_bytes();
            let server_ip = format!(
                "{}.{}.{}.{}",
                ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]
            );
            let address = format!("{}:{}", server_ip, event.server.port);

            if let Err(e) = client.connect(&address) {
                error!("Failed to connect to character server: {:?}", e);
            } else {
                info!("Connected to character server at {}", address);
                // Send CH_ENTER packet immediately after connection
                if let Err(e) = client.enter_server() {
                    error!("Failed to send CH_ENTER: {:?}", e);
                }
            }

            commands.insert_resource(client);
        }
    }
}
