use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        character::{
            protocol::{CharacterContext, CharacterProtocol},
            server_packets::HcCharDelete2AckPacket,
            types::CharDeletionError,
        },
        traits::{EventWriter, PacketHandler},
    },
};
use bevy::prelude::*;

/// Event emitted when character deletion succeeds
#[derive(Message, Debug, Clone)]
pub struct CharacterDeleted {
    pub char_id: u32,
}

/// Event emitted when character deletion fails
#[derive(Message, Debug, Clone)]
pub struct CharacterDeletionFailed {
    pub char_id: u32,
    pub error: CharDeletionError,
}

/// Handler for HC_CHAR_DELETE2_ACK packet
pub struct CharDelete2AckHandler;

impl PacketHandler<CharacterProtocol> for CharDelete2AckHandler {
    type Packet = HcCharDelete2AckPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut CharacterContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        if packet.is_success() {
            info!(
                "Character {} marked for deletion (delete_date: {})",
                packet.char_id, packet.delete_date
            );
            event_writer.send_event(Box::new(CharacterDeleted {
                char_id: packet.char_id,
            }));
            return Ok(());
        }

        let error = CharDeletionError::from(packet.result);
        warn!("Character {} deletion refused: {:?}", packet.char_id, error);
        event_writer.send_event(Box::new(CharacterDeletionFailed {
            char_id: packet.char_id,
            error,
        }));

        Ok(())
    }
}
