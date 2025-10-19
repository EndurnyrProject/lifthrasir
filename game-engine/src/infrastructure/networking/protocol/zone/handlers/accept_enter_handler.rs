use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        traits::{EventWriter, PacketHandler},
        zone::{
            protocol::{ZoneContext, ZoneProtocol},
            server_packets::ZcAcceptEnterPacket,
            types::SpawnData,
        },
    },
};
use bevy::prelude::*;

/// Event emitted when zone server accepts entry and player spawns
#[derive(Message, Debug, Clone)]
pub struct ZoneServerConnected {
    pub spawn_data: SpawnData,
}

/// Handler for ZC_ACCEPT_ENTER packet
///
/// Processes the zone server's acceptance of the client connection
/// and emits an event with spawn data including position, server tick,
/// and character size.
pub struct AcceptEnterHandler;

impl PacketHandler<ZoneProtocol> for AcceptEnterHandler {
    type Packet = ZcAcceptEnterPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        info!(
            "Zone server accepted entry! Spawning at ({}, {}) facing {}",
            packet.spawn_data.position.x,
            packet.spawn_data.position.y,
            packet.spawn_data.position.dir
        );

        debug!(
            "Server tick: {}, Size: {}x{}, Font: {}",
            packet.spawn_data.server_tick,
            packet.spawn_data.x_size,
            packet.spawn_data.y_size,
            packet.spawn_data.font
        );

        // Store spawn data in context
        context.set_spawn_data(packet.spawn_data.clone());

        // Emit event
        let event = ZoneServerConnected {
            spawn_data: packet.spawn_data,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
