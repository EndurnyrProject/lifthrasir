use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        character::{
            protocol::{CharacterContext, CharacterProtocol},
            server_packets::HcAckCharinfoPerPagePacket,
            types::CharacterInfo,
        },
        traits::{EventWriter, PacketHandler},
    },
};
use bevy::prelude::*;

/// Event emitted when character info page is received
#[derive(Message, Debug, Clone)]
pub struct CharacterInfoPageReceived {
    pub characters: Vec<CharacterInfo>,
}

/// Handler for HC_ACK_CHARINFO_PER_PAGE packet
pub struct AckCharinfoPerPageHandler;

impl PacketHandler<CharacterProtocol> for AckCharinfoPerPageHandler {
    type Packet = HcAckCharinfoPerPagePacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut CharacterContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!(
            "Character info page received with {} character(s)",
            packet.characters.len()
        );

        // Add characters to context
        context.add_characters(packet.characters.clone());

        let event = CharacterInfoPageReceived {
            characters: packet.characters,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
