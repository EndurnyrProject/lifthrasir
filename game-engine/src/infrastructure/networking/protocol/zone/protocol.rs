use super::{client_packets::*, server_packets::*, types::SpawnData};
use crate::infrastructure::networking::protocol::traits::{
    ClientPacket, PacketSize, Protocol, ServerPacket,
};
use bytes::Bytes;
use std::io;

/// Zone protocol definition
///
/// The zone protocol handles in-game communication once a player enters the game world.
/// This includes player spawning, movement, NPCs, monsters, items, chat, and all
/// gameplay interactions. This is the most complex protocol with hundreds of potential
/// packets, but we start with the essential ones for connecting and entering the world.
pub struct ZoneProtocol;

impl Protocol for ZoneProtocol {
    const NAME: &'static str = "Zone";

    type ClientPacket = ZoneClientPacket;
    type ServerPacket = ZoneServerPacket;
    type Context = ZoneContext;

    fn parse_server_packet(packet_id: u16, data: &[u8]) -> io::Result<Self::ServerPacket> {
        match packet_id {
            ZC_ACCEPT_ENTER => {
                let packet = ZcAcceptEnterPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcAcceptEnter(packet))
            }
            ZC_AID => {
                let packet = ZcAidPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcAid(packet))
            }
            ZC_REFUSE_ENTER => {
                let packet = ZcRefuseEnterPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcRefuseEnter(packet))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown zone packet ID: 0x{:04X}", packet_id),
            )),
        }
    }

    fn packet_size(packet_id: u16) -> PacketSize {
        match packet_id {
            ZC_ACCEPT_ENTER => PacketSize::Fixed(13),
            ZC_AID => PacketSize::Fixed(6),
            ZC_REFUSE_ENTER => PacketSize::Fixed(3),
            _ => PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            }, // Unknown - assume variable-length and try to skip
        }
    }
}

/// Context maintained during zone protocol processing
///
/// Tracks the player's state in the game world including spawn data,
/// account information, and connection state.
#[derive(Debug, Default)]
pub struct ZoneContext {
    /// Account ID for this session
    pub account_id: Option<u32>,

    /// Character ID for the active character
    pub character_id: Option<u32>,

    /// Spawn data received when entering the world
    pub spawn_data: Option<SpawnData>,

    /// Whether we received the ZC_AID acknowledgment
    pub received_aid: bool,

    /// Whether we've successfully entered the game world
    pub entered_world: bool,

    /// Server tick from last update (for synchronization)
    pub server_tick: u32,
}

impl ZoneContext {
    /// Create a new zone context
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize context with session data from character server
    pub fn with_session(account_id: u32, character_id: u32) -> Self {
        Self {
            account_id: Some(account_id),
            character_id: Some(character_id),
            ..Default::default()
        }
    }

    /// Set spawn data after entering the world
    pub fn set_spawn_data(&mut self, spawn_data: SpawnData) {
        self.server_tick = spawn_data.server_tick;
        self.spawn_data = Some(spawn_data);
        self.entered_world = true;
    }

    /// Mark AID as received
    pub fn acknowledge_aid(&mut self, account_id: u32) {
        self.account_id = Some(account_id);
        self.received_aid = true;
    }

    /// Check if fully connected and ready
    pub fn is_ready(&self) -> bool {
        self.entered_world && self.received_aid
    }

    /// Reset context for new connection
    pub fn reset(&mut self) {
        self.spawn_data = None;
        self.received_aid = false;
        self.entered_world = false;
        self.server_tick = 0;
    }
}

/// Enum of all client packets for zone protocol
#[derive(Debug, Clone)]
pub enum ZoneClientPacket {
    CzEnter2(CzEnter2Packet),
    CzNotifyActorinit(CzNotifyActorinitPacket),
}

impl ClientPacket for ZoneClientPacket {
    const PACKET_ID: u16 = 0; // Not used for enums

    fn serialize(&self) -> Bytes {
        match self {
            Self::CzEnter2(p) => p.serialize(),
            Self::CzNotifyActorinit(p) => p.serialize(),
        }
    }

    fn packet_id(&self) -> u16 {
        match self {
            Self::CzEnter2(_) => CZ_ENTER2,
            Self::CzNotifyActorinit(_) => CZ_NOTIFY_ACTORINIT,
        }
    }
}

/// Enum of all server packets for zone protocol
#[derive(Debug, Clone)]
pub enum ZoneServerPacket {
    ZcAcceptEnter(ZcAcceptEnterPacket),
    ZcAid(ZcAidPacket),
    ZcRefuseEnter(ZcRefuseEnterPacket),
}

impl ServerPacket for ZoneServerPacket {
    const PACKET_ID: u16 = 0; // Not used for enums

    fn parse(_data: &[u8]) -> io::Result<Self> {
        unreachable!("Use Protocol::parse_server_packet instead")
    }

    fn packet_id(&self) -> u16 {
        match self {
            Self::ZcAcceptEnter(_) => ZC_ACCEPT_ENTER,
            Self::ZcAid(_) => ZC_AID,
            Self::ZcRefuseEnter(_) => ZC_REFUSE_ENTER,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zone_context_creation() {
        let context = ZoneContext::new();
        assert!(!context.is_ready());
        assert!(!context.entered_world);
        assert!(!context.received_aid);
    }

    #[test]
    fn test_zone_context_with_session() {
        let context = ZoneContext::with_session(12345, 67890);
        assert_eq!(context.account_id, Some(12345));
        assert_eq!(context.character_id, Some(67890));
        assert!(!context.is_ready());
    }

    #[test]
    fn test_zone_context_ready_state() {
        let mut context = ZoneContext::with_session(12345, 67890);

        // Not ready yet
        assert!(!context.is_ready());

        // Set spawn data
        let spawn_data = SpawnData::new(
            1000,
            crate::infrastructure::networking::protocol::zone::types::Position::new(100, 100, 0),
            5,
            5,
            0,
        );
        context.set_spawn_data(spawn_data);
        assert!(context.entered_world);
        assert!(!context.is_ready()); // Still need AID

        // Acknowledge AID
        context.acknowledge_aid(12345);
        assert!(context.is_ready()); // Now ready
    }
}
