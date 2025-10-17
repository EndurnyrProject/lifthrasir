use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        character::{
            protocol::{CharacterContext, CharacterProtocol},
            server_packets::{HcAcceptDeletecharPacket, HcRefuseDeletecharPacket},
            types::CharDeletionError,
        },
        traits::{EventWriter, PacketHandler},
    },
};
use bevy::prelude::*;

/// Event emitted when character deletion succeeds
#[derive(Event, Debug, Clone)]
pub struct CharacterDeleted {
    pub char_id: u32,
}

/// Event emitted when character deletion fails
#[derive(Event, Debug, Clone)]
pub struct CharacterDeletionFailed {
    pub error: CharDeletionError,
}

/// Handler for HC_ACCEPT_DELETECHAR packet
pub struct AcceptDeletecharHandler;

impl PacketHandler<CharacterProtocol> for AcceptDeletecharHandler {
    type Packet = HcAcceptDeletecharPacket;

    fn handle(
        &self,
        _packet: Self::Packet,
        _context: &mut CharacterContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        info!("Character deleted successfully");

        // Note: We don't have char_id in the packet, so we'd need to track
        // which character was being deleted in context if needed
        let event = CharacterDeleted { char_id: 0 };

        event_writer.send_event(Box::new(event));

        // Optionally refresh character list
        // context.clear_characters();

        Ok(())
    }
}

/// Handler for HC_REFUSE_DELETECHAR packet
pub struct RefuseDeletecharHandler;

impl PacketHandler<CharacterProtocol> for RefuseDeletecharHandler {
    type Packet = HcRefuseDeletecharPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut CharacterContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        warn!("Character deletion refused: {:?}", packet.error);

        let event = CharacterDeletionFailed {
            error: packet.error,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
