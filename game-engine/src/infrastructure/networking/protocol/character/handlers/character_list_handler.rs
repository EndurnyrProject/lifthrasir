use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        character::{
            protocol::{CharacterContext, CharacterProtocol},
            server_packets::HcCharacterListPacket,
            types::CharacterSlotInfo,
        },
        traits::{EventWriter, PacketHandler},
    },
};
use bevy::prelude::*;

/// Event emitted when character slot information is received
#[derive(Event, Debug, Clone)]
pub struct CharacterSlotInfoReceived {
    pub slot_info: CharacterSlotInfo,
}

/// Handler for HC_CHARACTER_LIST packet
pub struct CharacterListHandler;

impl PacketHandler<CharacterProtocol> for CharacterListHandler {
    type Packet = HcCharacterListPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut CharacterContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!(
            "Character slot info: normal={}, premium={}, billing={}, producible={}, valid={}",
            packet.slot_info.normal_slots,
            packet.slot_info.premium_slots,
            packet.slot_info.billing_slots,
            packet.slot_info.producible_slots,
            packet.slot_info.valid_slots
        );

        let event = CharacterSlotInfoReceived {
            slot_info: packet.slot_info,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
