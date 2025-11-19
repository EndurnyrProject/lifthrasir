use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        traits::{EventWriter, PacketHandler},
        zone::{
            protocol::{ZoneContext, ZoneProtocol},
            server_packets::zc_notify_chat::ZcNotifyChatPacket,
        },
    },
};
use bevy::prelude::*;

#[derive(Message, Debug, Clone)]
pub struct ChatReceived {
    pub gid: u32,
    pub message: String,
}

pub struct ChatHandler;

impl PacketHandler<ZoneProtocol> for ChatHandler {
    type Packet = ZcNotifyChatPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        info!("[CHAT] {}: {}", packet.gid, packet.message);
        event_writer.send_event(Box::new(ChatReceived {
            gid: packet.gid,
            message: packet.message,
        }));
        Ok(())
    }
}
