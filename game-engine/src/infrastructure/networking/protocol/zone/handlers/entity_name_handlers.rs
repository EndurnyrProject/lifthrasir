use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        traits::{EventWriter, PacketHandler},
        zone::{
            protocol::{ZoneContext, ZoneProtocol},
            server_packets::{ZcAckReqnamePacket, ZcAckReqnameallPacket},
        },
    },
};
use bevy::prelude::*;

#[derive(Message, Debug, Clone)]
pub struct EntityNameReceived {
    pub char_id: u32,
    pub name: String,
}

#[derive(Message, Debug, Clone)]
pub struct EntityNameAllReceived {
    pub gid: u32,
    pub name: String,
    pub party_name: String,
    pub guild_name: String,
    pub position_name: String,
}

pub struct ReqnameHandler;

impl PacketHandler<ZoneProtocol> for ReqnameHandler {
    type Packet = ZcAckReqnamePacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!("Entity name received: {} (ID: {})", packet.name, packet.char_id);

        let event = EntityNameReceived {
            char_id: packet.char_id,
            name: packet.name,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}

pub struct ReqnameallHandler;

impl PacketHandler<ZoneProtocol> for ReqnameallHandler {
    type Packet = ZcAckReqnameallPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!(
            "Entity full name received: {} (ID: {}), Party: {}, Guild: {}, Position: {}",
            packet.name, packet.gid, packet.party_name, packet.guild_name, packet.position_name
        );

        let event = EntityNameAllReceived {
            gid: packet.gid,
            name: packet.name,
            party_name: packet.party_name,
            guild_name: packet.guild_name,
            position_name: packet.position_name,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
