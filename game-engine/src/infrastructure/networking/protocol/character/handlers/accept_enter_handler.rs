use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        character::{
            protocol::{CharacterContext, CharacterProtocol},
            server_packets::HcAcceptEnterPacket,
            types::CharacterInfo,
        },
        traits::{EventWriter, PacketHandler},
    },
};
use bevy::prelude::*;

/// Event emitted when character server connection is accepted
#[derive(Message, Debug, Clone)]
pub struct CharacterServerConnected {
    pub max_slots: u8,
    pub available_slots: u8,
    pub premium_slots: u8,
    pub characters: Vec<CharacterInfo>,
}

/// Handler for HC_ACCEPT_ENTER packet
///
/// Processes the character server's acceptance of the client connection
/// and emits an event with the character list.
pub struct AcceptEnterHandler;

impl PacketHandler<CharacterProtocol> for AcceptEnterHandler {
    type Packet = HcAcceptEnterPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut CharacterContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        info!(
            "Character server connected! {} characters available (max: {}, available: {}, premium: {})",
            packet.characters.len(),
            packet.max_slots,
            packet.available_slots,
            packet.premium_slots
        );

        // Log character names for debugging
        for (idx, character) in packet.characters.iter().enumerate() {
            debug!(
                "  [{}] {} (ID: {}, Lv: {}, Job: {})",
                idx, character.name, character.char_id, character.base_level, character.class
            );
        }

        // Store characters in context
        context.clear_characters();
        context.add_characters(packet.characters.clone());

        // Emit event
        let event = CharacterServerConnected {
            max_slots: packet.max_slots,
            available_slots: packet.available_slots,
            premium_slots: packet.premium_slots,
            characters: packet.characters,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
