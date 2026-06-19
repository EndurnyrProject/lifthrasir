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
        // The server answers a CH_CHARLIST_REQ refresh with the full list in a
        // single packet. When the list is exactly 3 characters it appends a
        // trailing empty packet (Gravity's finalization quirk); only the first
        // response is authoritative, so ignore anything we didn't ask for.
        if !context.awaiting_charlist {
            debug!(
                "Ignoring unrequested/trailing HC_ACK_CHARINFO_PER_PAGE ({} character(s))",
                packet.characters.len()
            );
            return Ok(());
        }

        debug!(
            "Character list refreshed with {} character(s)",
            packet.characters.len()
        );

        context.awaiting_charlist = false;
        context.set_characters(packet.characters.clone());

        let event = CharacterInfoPageReceived {
            characters: packet.characters,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
