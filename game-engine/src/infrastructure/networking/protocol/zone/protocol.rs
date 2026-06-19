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
            ZC_ACK_REQNAME => {
                let packet = ZcAckReqnamePacket::parse(data)?;
                Ok(ZoneServerPacket::ZcAckReqname(packet))
            }
            ZC_ACK_REQNAMEALL => {
                let packet = ZcAckReqnameallPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcAckReqnameall(packet))
            }
            ZC_AID => {
                let packet = ZcAidPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcAid(packet))
            }
            ZC_REFUSE_ENTER => {
                let packet = ZcRefuseEnterPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcRefuseEnter(packet))
            }
            ZC_NOTIFY_PLAYERMOVE => {
                let packet = ZcNotifyPlayermovePacket::parse(data)?;
                Ok(ZoneServerPacket::ZcNotifyPlayermove(packet))
            }
            ZC_NOTIFY_MOVE_STOP => {
                let packet = ZcNotifyMoveStopPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcNotifyMoveStop(packet))
            }
            ZC_NOTIFY_STANDENTRY => {
                let packet = ZcNotifyStandentryPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcNotifyStandentry(packet))
            }
            ZC_NOTIFY_NEWENTRY => {
                let packet = ZcNotifyNewentryPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcNotifyNewentry(packet))
            }
            ZC_NOTIFY_MOVEENTRY => {
                let packet = ZcNotifyMoveentryPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcNotifyMoveentry(packet))
            }
            ZC_NOTIFY_VANISH => {
                let packet = ZcNotifyVanishPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcNotifyVanish(packet))
            }
            ZC_NOTIFY_TIME => {
                let packet = ZcNotifyTimePacket::parse(data)?;
                Ok(ZoneServerPacket::ZcNotifyTime(packet))
            }
            ZC_NOTIFY_TIME2 => {
                let packet = ZcNotifyTime2Packet::parse(data)?;
                Ok(ZoneServerPacket::ZcNotifyTime2(packet))
            }
            ZC_PAR_CHANGE => {
                let packet = ZcParChangePacket::parse(data)?;
                Ok(ZoneServerPacket::ZcParChange(packet))
            }
            ZC_LONGPAR_CHANGE => {
                let packet = ZcLongparChangePacket::parse(data)?;
                Ok(ZoneServerPacket::ZcLongparChange(packet))
            }
            ZC_INVENTORY_START => {
                let packet = ZcInventoryStartPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcInventoryStart(packet))
            }
            ZC_INVENTORY_ITEMLIST_NORMAL => {
                let packet = ZcInventoryItemlistNormalPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcInventoryItemlistNormal(packet))
            }
            ZC_INVENTORY_ITEMLIST_EQUIP => {
                let packet = ZcInventoryItemlistEquipPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcInventoryItemlistEquip(packet))
            }
            ZC_INVENTORY_END => {
                let packet = ZcInventoryEndPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcInventoryEnd(packet))
            }
            ZC_NOTIFY_CHAT => {
                let packet = ZcNotifyChatPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcNotifyChat(packet))
            }
            ZC_NOTIFY_ACT => {
                let packet = ZcNotifyActPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcNotifyAct(packet))
            }
            ZC_HP_INFO => {
                let packet = ZcHpInfoPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcHpInfo(packet))
            }
            ZC_STATUS_CHANGE_ACK => {
                let packet = ZcStatusChangeAckPacket::parse(data)?;
                Ok(ZoneServerPacket::ZcStatusChangeAck(packet))
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
            ZC_ACK_REQNAME => PacketSize::Fixed(30),
            ZC_ACK_REQNAMEALL => PacketSize::Fixed(102),
            ZC_AID => PacketSize::Fixed(6),
            ZC_REFUSE_ENTER => PacketSize::Fixed(3),
            ZC_NOTIFY_PLAYERMOVE => PacketSize::Fixed(12),
            ZC_NOTIFY_MOVE_STOP => PacketSize::Fixed(10),
            ZC_NOTIFY_STANDENTRY => PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            },
            ZC_NOTIFY_NEWENTRY => PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            },
            ZC_NOTIFY_MOVEENTRY => PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            },
            ZC_NOTIFY_VANISH => PacketSize::Fixed(7),
            ZC_NOTIFY_TIME => PacketSize::Fixed(6),
            ZC_NOTIFY_TIME2 => PacketSize::Fixed(6),
            ZC_PAR_CHANGE => PacketSize::Fixed(8),
            ZC_LONGPAR_CHANGE => PacketSize::Fixed(8),
            ZC_INVENTORY_START => PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            },
            ZC_INVENTORY_ITEMLIST_NORMAL => PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            },
            ZC_INVENTORY_ITEMLIST_EQUIP => PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            },
            ZC_INVENTORY_END => PacketSize::Fixed(4),
            ZC_NOTIFY_CHAT => PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            },
            ZC_NOTIFY_ACT => PacketSize::Fixed(34),
            ZC_HP_INFO => PacketSize::Fixed(14),
            ZC_STATUS_CHANGE_ACK => PacketSize::Fixed(6),
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

    /// Server time offset in milliseconds (server_time = local_time + time_offset)
    pub time_offset: i64,

    /// Last time sync request timestamp
    pub last_time_sync: Option<std::time::Instant>,
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
        self.time_offset = 0;
        self.last_time_sync = None;
    }

    /// Update time offset based on server response
    ///
    /// # Arguments
    ///
    /// * `server_time` - Current server time in milliseconds
    /// * `client_time` - Client time when request was sent in milliseconds
    pub fn update_time_offset(&mut self, server_time: u32, client_time: u32) {
        // Calculate round-trip time and assume half was spent in transit
        let rtt = crate::utils::time::current_milliseconds().wrapping_sub(client_time);
        let estimated_server_time = server_time.wrapping_add(rtt / 2);
        let local_time = crate::utils::time::current_milliseconds();

        // Calculate offset (server - local)
        self.time_offset = estimated_server_time.wrapping_sub(local_time) as i32 as i64;
        self.last_time_sync = Some(std::time::Instant::now());
    }

    /// Get current server time based on synchronized offset
    ///
    /// # Returns
    ///
    /// Estimated server time in milliseconds
    pub fn get_server_time(&self) -> u32 {
        let local_ms = crate::utils::time::current_milliseconds();
        local_ms.wrapping_add(self.time_offset as i32 as u32)
    }

    /// Check if time sync is needed
    ///
    /// Returns true if we haven't synced in the last 30 seconds
    pub fn needs_time_sync(&self) -> bool {
        match self.last_time_sync {
            None => true,
            Some(last_sync) => last_sync.elapsed().as_secs() >= 30,
        }
    }
}

/// Enum of all client packets for zone protocol
#[derive(Debug, Clone)]
pub enum ZoneClientPacket {
    CzEnter2(CzEnter2Packet),
    CzNotifyActorinit(CzNotifyActorinitPacket),
    CzReqname2(CzReqname2Packet),
    CzRequestAct2(CzRequestAct2Packet),
    CzRequestMove2(CzRequestMove2Packet),
    CzRequestTime2(CzRequestTime2Packet),
    CzRequestChat(CzRequestChatPacket),
    CzStatusChange(CzStatusChangePacket),
}

impl ClientPacket for ZoneClientPacket {
    const PACKET_ID: u16 = 0; // Not used for enums

    fn serialize(&self) -> Bytes {
        match self {
            Self::CzEnter2(p) => p.serialize(),
            Self::CzNotifyActorinit(p) => p.serialize(),
            Self::CzReqname2(p) => p.serialize(),
            Self::CzRequestAct2(p) => p.serialize(),
            Self::CzRequestMove2(p) => p.serialize(),
            Self::CzRequestTime2(p) => p.serialize(),
            Self::CzRequestChat(p) => p.serialize(),
            Self::CzStatusChange(p) => p.serialize(),
        }
    }

    fn packet_id(&self) -> u16 {
        match self {
            Self::CzEnter2(_) => CZ_ENTER2,
            Self::CzNotifyActorinit(_) => CZ_NOTIFY_ACTORINIT,
            Self::CzReqname2(_) => CZ_REQNAME2,
            Self::CzRequestAct2(_) => CZ_REQUEST_ACT2,
            Self::CzRequestMove2(_) => CZ_REQUEST_MOVE2,
            Self::CzRequestTime2(_) => CZ_REQUEST_TIME2,
            Self::CzRequestChat(_) => CZ_REQUEST_CHAT,
            Self::CzStatusChange(_) => CZ_STATUS_CHANGE,
        }
    }
}

/// Enum of all server packets for zone protocol
#[derive(Debug, Clone)]
pub enum ZoneServerPacket {
    ZcAcceptEnter(ZcAcceptEnterPacket),
    ZcAckReqname(ZcAckReqnamePacket),
    ZcAckReqnameall(ZcAckReqnameallPacket),
    ZcAid(ZcAidPacket),
    ZcRefuseEnter(ZcRefuseEnterPacket),
    ZcNotifyPlayermove(ZcNotifyPlayermovePacket),
    ZcNotifyMoveStop(ZcNotifyMoveStopPacket),
    ZcNotifyStandentry(ZcNotifyStandentryPacket),
    ZcNotifyNewentry(ZcNotifyNewentryPacket),
    ZcNotifyMoveentry(ZcNotifyMoveentryPacket),
    ZcNotifyVanish(ZcNotifyVanishPacket),
    ZcNotifyTime(ZcNotifyTimePacket),
    ZcNotifyTime2(ZcNotifyTime2Packet),
    ZcParChange(ZcParChangePacket),
    ZcLongparChange(ZcLongparChangePacket),
    ZcInventoryStart(ZcInventoryStartPacket),
    ZcInventoryItemlistNormal(ZcInventoryItemlistNormalPacket),
    ZcInventoryItemlistEquip(ZcInventoryItemlistEquipPacket),
    ZcInventoryEnd(ZcInventoryEndPacket),
    ZcNotifyChat(ZcNotifyChatPacket),
    ZcNotifyAct(ZcNotifyActPacket),
    ZcHpInfo(ZcHpInfoPacket),
    ZcStatusChangeAck(ZcStatusChangeAckPacket),
}

impl ServerPacket for ZoneServerPacket {
    const PACKET_ID: u16 = 0; // Not used for enums

    fn parse(_data: &[u8]) -> io::Result<Self> {
        unreachable!("Use Protocol::parse_server_packet instead")
    }

    fn packet_id(&self) -> u16 {
        match self {
            Self::ZcAcceptEnter(_) => ZC_ACCEPT_ENTER,
            Self::ZcAckReqname(_) => ZC_ACK_REQNAME,
            Self::ZcAckReqnameall(_) => ZC_ACK_REQNAMEALL,
            Self::ZcAid(_) => ZC_AID,
            Self::ZcRefuseEnter(_) => ZC_REFUSE_ENTER,
            Self::ZcNotifyPlayermove(_) => ZC_NOTIFY_PLAYERMOVE,
            Self::ZcNotifyMoveStop(_) => ZC_NOTIFY_MOVE_STOP,
            Self::ZcNotifyStandentry(_) => ZC_NOTIFY_STANDENTRY,
            Self::ZcNotifyNewentry(_) => ZC_NOTIFY_NEWENTRY,
            Self::ZcNotifyMoveentry(_) => ZC_NOTIFY_MOVEENTRY,
            Self::ZcNotifyVanish(_) => ZC_NOTIFY_VANISH,
            Self::ZcNotifyTime(_) => ZC_NOTIFY_TIME,
            Self::ZcNotifyTime2(_) => ZC_NOTIFY_TIME2,
            Self::ZcParChange(_) => ZC_PAR_CHANGE,
            Self::ZcLongparChange(_) => ZC_LONGPAR_CHANGE,
            Self::ZcInventoryStart(_) => ZC_INVENTORY_START,
            Self::ZcInventoryItemlistNormal(_) => ZC_INVENTORY_ITEMLIST_NORMAL,
            Self::ZcInventoryItemlistEquip(_) => ZC_INVENTORY_ITEMLIST_EQUIP,
            Self::ZcInventoryEnd(_) => ZC_INVENTORY_END,
            Self::ZcNotifyChat(_) => ZC_NOTIFY_CHAT,
            Self::ZcNotifyAct(_) => ZC_NOTIFY_ACT,
            Self::ZcHpInfo(_) => ZC_HP_INFO,
            Self::ZcStatusChangeAck(_) => ZC_STATUS_CHANGE_ACK,
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

    fn variable_packet(id: u16, body: &[u8]) -> Vec<u8> {
        let len = (4 + body.len()) as u16;
        let mut data = id.to_le_bytes().to_vec();
        data.extend_from_slice(&len.to_le_bytes());
        data.extend_from_slice(body);
        data
    }

    #[test]
    fn parses_inventory_start() {
        let data = variable_packet(ZC_INVENTORY_START, &[0, b'P', b'C', 0]);
        let packet = ZoneProtocol::parse_server_packet(ZC_INVENTORY_START, &data).expect("parse");
        match packet {
            ZoneServerPacket::ZcInventoryStart(p) => assert_eq!(p.name, "PC"),
            other => panic!("wrong variant: {:?}", other),
        }
    }

    #[test]
    fn parses_inventory_itemlist_normal() {
        let item = [0u8; 34];
        let data = variable_packet(ZC_INVENTORY_ITEMLIST_NORMAL, &[&[0u8][..], &item].concat());
        let packet =
            ZoneProtocol::parse_server_packet(ZC_INVENTORY_ITEMLIST_NORMAL, &data).expect("parse");
        match packet {
            ZoneServerPacket::ZcInventoryItemlistNormal(p) => assert_eq!(p.items.len(), 1),
            other => panic!("wrong variant: {:?}", other),
        }
    }

    #[test]
    fn parses_inventory_itemlist_equip() {
        let item = [0u8; 67];
        let data = variable_packet(ZC_INVENTORY_ITEMLIST_EQUIP, &[&[0u8][..], &item].concat());
        let packet =
            ZoneProtocol::parse_server_packet(ZC_INVENTORY_ITEMLIST_EQUIP, &data).expect("parse");
        match packet {
            ZoneServerPacket::ZcInventoryItemlistEquip(p) => assert_eq!(p.items.len(), 1),
            other => panic!("wrong variant: {:?}", other),
        }
    }

    #[test]
    fn parses_inventory_end() {
        let data = [0x0B, 0x0B, 0x00, 0x00];
        let packet = ZoneProtocol::parse_server_packet(ZC_INVENTORY_END, &data).expect("parse");
        match packet {
            ZoneServerPacket::ZcInventoryEnd(p) => assert_eq!(p.flag, 0),
            other => panic!("wrong variant: {:?}", other),
        }
    }

    fn assert_variable(id: u16) {
        match ZoneProtocol::packet_size(id) {
            PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            } => {}
            other => panic!("expected variable size for 0x{:04X}, got {:?}", id, other),
        }
    }

    #[test]
    fn inventory_packet_sizes() {
        assert_variable(ZC_INVENTORY_START);
        assert_variable(ZC_INVENTORY_ITEMLIST_NORMAL);
        assert_variable(ZC_INVENTORY_ITEMLIST_EQUIP);
        match ZoneProtocol::packet_size(ZC_INVENTORY_END) {
            PacketSize::Fixed(4) => {}
            other => panic!("expected Fixed(4) for END, got {:?}", other),
        }
    }
}
