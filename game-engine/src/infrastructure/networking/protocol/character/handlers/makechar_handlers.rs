use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        character::{
            protocol::{CharacterContext, CharacterProtocol},
            server_packets::{HcAcceptMakecharPacket, HcRefuseMakecharPacket},
            types::{CharCreationError, CharacterInfo},
        },
        traits::{EventWriter, PacketHandler},
    },
};
use bevy::prelude::*;

/// Event emitted when character creation succeeds
#[derive(Message, Debug, Clone)]
pub struct CharacterCreated {
    pub character: CharacterInfo,
}

/// Event emitted when character creation fails
#[derive(Message, Debug, Clone)]
pub struct CharacterCreationFailed {
    pub error: CharCreationError,
}

/// Handler for HC_ACCEPT_MAKECHAR packet
pub struct AcceptMakecharHandler;

impl PacketHandler<CharacterProtocol> for AcceptMakecharHandler {
    type Packet = HcAcceptMakecharPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut CharacterContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        info!(
            "Character created successfully: {} (ID: {})",
            packet.character.name, packet.character.char_id
        );

        // Add new character to context
        context.add_characters(vec![packet.character.clone()]);

        let event = CharacterCreated {
            character: packet.character,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}

/// Handler for HC_REFUSE_MAKECHAR packet
pub struct RefuseMakecharHandler;

impl PacketHandler<CharacterProtocol> for RefuseMakecharHandler {
    type Packet = HcRefuseMakecharPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut CharacterContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        warn!("Character creation refused: {:?}", packet.error);

        let event = CharacterCreationFailed {
            error: packet.error,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
