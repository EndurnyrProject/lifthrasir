use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        traits::{EventWriter, PacketHandler},
        zone::{
            protocol::{ZoneContext, ZoneProtocol},
            server_packets::ZcAidPacket,
        },
    },
};
use bevy::prelude::*;

/// Event emitted when account ID is received from zone server
#[derive(Message, Debug, Clone)]
pub struct AccountIdReceived {
    pub account_id: u32,
}

/// Handler for ZC_AID packet
///
/// Processes the account ID confirmation from the zone server.
/// This is sent after successfully entering the zone.
pub struct AidHandler;

impl PacketHandler<ZoneProtocol> for AidHandler {
    type Packet = ZcAidPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        info!("Account ID received: {}", packet.account_id);

        // Store account ID in context
        context.acknowledge_aid(packet.account_id);

        // Emit event
        let event = AccountIdReceived {
            account_id: packet.account_id,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
