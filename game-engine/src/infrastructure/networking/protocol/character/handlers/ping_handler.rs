use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        character::{
            protocol::{CharacterContext, CharacterProtocol},
            server_packets::HcPingPacket,
        },
        traits::{EventWriter, PacketHandler},
    },
};
use bevy::prelude::*;

/// Event emitted when ping response is received
#[derive(Message, Debug, Clone, Copy)]
pub struct PingReceived;

/// Handler for HC_PING packet
pub struct PingHandler;

impl PacketHandler<CharacterProtocol> for PingHandler {
    type Packet = HcPingPacket;

    fn handle(
        &self,
        _packet: Self::Packet,
        _context: &mut CharacterContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        trace!("Ping received from character server");

        let event = PingReceived;
        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
