use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_message, auto_add_system};
use bevy_quinnet::client::client_connected;
use bevy_quinnet::client::QuinnetClient;

use super::connection::QuicConnection;
use super::envelope::Body;

/// A single decoded inbound message drained from the shared QUIC connection.
///
/// Draining bevy_quinnet's receive buffers is destructive: `receive_payload`
/// *pops* each payload, so a channel's messages can only be consumed once. With
/// several flow systems (login, character, zone) each draining the connection
/// directly, whichever Bevy scheduled first stole every channel's messages and
/// the others saw nothing.
///
/// [`drain_incoming`] is now the only drainer. It republishes each payload as
/// this buffered [`Message`], which any number of flow systems read
/// independently via `MessageReader` — each keeps its own cursor, so all of
/// them see every message.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct IncomingMessage {
    pub channel: u8,
    pub body: Body,
}

/// Drains every channel of the default connection once per frame and
/// republishes the decoded bodies as [`IncomingMessage`]s for the flow systems.
///
/// Runs in `PreUpdate` so the `Update` flow consumers see this frame's payloads.
#[auto_add_system(
    plugin = crate::app::authentication_plugin::AuthenticationPlugin,
    schedule = PreUpdate,
    config(run_if = client_connected)
)]
pub fn drain_incoming(mut client: ResMut<QuinnetClient>, mut out: MessageWriter<IncomingMessage>) {
    for (channel, body) in QuicConnection::drain(client.connection_mut()) {
        out.write(IncomingMessage { channel, body });
    }
}
