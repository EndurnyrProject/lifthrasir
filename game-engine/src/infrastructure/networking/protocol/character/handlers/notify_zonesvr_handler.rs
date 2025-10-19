use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        character::{
            protocol::{CharacterContext, CharacterProtocol},
            server_packets::HcNotifyZonesvrPacket,
            types::ZoneServerInfo,
        },
        traits::{EventWriter, PacketHandler},
    },
};
use bevy::prelude::*;

/// Event emitted when zone server connection info is received
#[derive(Message, Debug, Clone)]
pub struct ZoneServerInfoReceived {
    pub zone_server_info: ZoneServerInfo,
}

/// Handler for HC_NOTIFY_ZONESVR packet
///
/// Processes zone server connection information after character selection
/// and emits an event for connecting to the zone (map) server.
pub struct NotifyZonesvrHandler;

impl PacketHandler<CharacterProtocol> for NotifyZonesvrHandler {
    type Packet = HcNotifyZonesvrPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut CharacterContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        info!(
            "Zone server info received: {} at {}:{}",
            packet.zone_server_info.map_name,
            packet.zone_server_info.ip_string(),
            packet.zone_server_info.port
        );

        // Store zone server info in context
        context.set_zone_server(packet.zone_server_info.clone());

        // Emit event
        let event = ZoneServerInfoReceived {
            zone_server_info: packet.zone_server_info,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
