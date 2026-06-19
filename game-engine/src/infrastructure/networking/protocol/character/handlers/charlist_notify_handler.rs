use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        character::{
            protocol::{CharacterContext, CharacterProtocol},
            server_packets::HcCharlistNotifyPacket,
        },
        traits::{EventWriter, PacketHandler},
    },
};
use bevy::prelude::*;

/// Handler for HC_CHARLIST_NOTIFY packet
///
/// Records the character-select display-page count (`char_slots / 3`) on the
/// context so the char-select screen can lay slots out in pages of 3, matching
/// rAthena's `chclif_charlist_notify`. The full character list itself arrives in
/// HC_ACCEPT_ENTER; this packet carries no character data.
pub struct CharlistNotifyHandler;

impl PacketHandler<CharacterProtocol> for CharlistNotifyHandler {
    type Packet = HcCharlistNotifyPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut CharacterContext,
        _event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!(
            "Character list notify: {} display page(s)",
            packet.page_count
        );

        context.list_page_count = packet.page_count;

        Ok(())
    }
}
