use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        character::{
            protocol::{CharacterContext, CharacterProtocol},
            server_packets::HcBlockCharacterPacket,
            types::BlockedCharacterEntry,
        },
        traits::{EventWriter, PacketHandler},
    },
};
use bevy::prelude::*;

/// Event emitted when blocked character list is received
#[derive(Message, Debug, Clone)]
pub struct BlockedCharactersReceived {
    pub blocked_chars: Vec<BlockedCharacterEntry>,
}

/// Handler for HC_BLOCK_CHARACTER packet
pub struct BlockCharacterHandler;

impl PacketHandler<CharacterProtocol> for BlockCharacterHandler {
    type Packet = HcBlockCharacterPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut CharacterContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        if !packet.blocked_chars.is_empty() {
            warn!(
                "Received {} blocked character(s)",
                packet.blocked_chars.len()
            );

            for blocked in &packet.blocked_chars {
                debug!(
                    "  Character ID {} blocked until: {}",
                    blocked.char_id, blocked.expire_date
                );
            }
        }

        let event = BlockedCharactersReceived {
            blocked_chars: packet.blocked_chars,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
