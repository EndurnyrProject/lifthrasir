use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        character::{
            protocol::{CharacterContext, CharacterProtocol},
            server_packets::HcSecondPasswdLoginPacket,
            types::SecondPasswordState,
        },
        traits::{EventWriter, PacketHandler},
    },
};
use bevy::prelude::*;

/// Event emitted when second password/pincode is requested
#[derive(Message, Debug, Clone)]
pub struct SecondPasswordRequested {
    pub seed: u32,
    pub account_id: u32,
    pub state: SecondPasswordState,
}

/// Handler for HC_SECOND_PASSWD_LOGIN packet
pub struct SecondPasswdLoginHandler;

impl PacketHandler<CharacterProtocol> for SecondPasswdLoginHandler {
    type Packet = HcSecondPasswdLoginPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut CharacterContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        info!(
            "Second password state: {} ({})",
            packet.state.description(),
            packet.state.as_u16()
        );

        let event = SecondPasswordRequested {
            seed: packet.seed,
            account_id: packet.account_id,
            state: packet.state,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
