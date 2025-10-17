use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        traits::{EventWriter, PacketHandler},
        zone::{
            protocol::{ZoneContext, ZoneProtocol},
            server_packets::ZcRefuseEnterPacket,
            types::ZoneEntryError,
        },
    },
};
use bevy::prelude::*;

/// Event emitted when zone entry is refused
#[derive(Event, Debug, Clone)]
pub struct ZoneEntryRefused {
    pub error: ZoneEntryError,
    pub error_description: String,
}

/// Handler for ZC_REFUSE_ENTER packet
///
/// Processes zone entry refusal from the server.
/// This indicates that the player cannot enter the zone for some reason.
pub struct RefuseEnterHandler;

impl PacketHandler<ZoneProtocol> for RefuseEnterHandler {
    type Packet = ZcRefuseEnterPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        error!(
            "Zone entry refused: {:?} - {}",
            packet.error,
            packet.error_description()
        );

        // Emit event
        let event = ZoneEntryRefused {
            error: packet.error,
            error_description: packet.error_description().to_string(),
        };

        event_writer.send_event(Box::new(event));

        // Return error to disconnect
        Err(NetworkError::ConnectionFailed(format!(
            "Zone entry refused: {}",
            packet.error_description()
        )))
    }
}
