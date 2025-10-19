use super::{
    client_packets::*,
    server_packets::*,
    types::{CharacterInfo, ZoneServerInfo},
};
use crate::infrastructure::networking::protocol::traits::{
    ClientPacket, PacketSize, Protocol, ServerPacket,
};
use bytes::Bytes;
use std::io;

/// Character protocol definition
///
/// The character protocol handles character management and selection.
/// It's more complex than login with multiple packet types for:
/// - Character listing and selection
/// - Character creation and deletion
/// - Zone server redirection
/// - Keep-alive pings
pub struct CharacterProtocol;

impl Protocol for CharacterProtocol {
    const NAME: &'static str = "Character";

    type ClientPacket = CharacterClientPacket;
    type ServerPacket = CharacterServerPacket;
    type Context = CharacterContext;

    fn parse_server_packet(packet_id: u16, data: &[u8]) -> io::Result<Self::ServerPacket> {
        match packet_id {
            HC_ACCEPT_ENTER => {
                let packet = HcAcceptEnterPacket::parse(data)?;
                Ok(CharacterServerPacket::HcAcceptEnter(packet))
            }
            HC_NOTIFY_ZONESVR => {
                let packet = HcNotifyZonesvrPacket::parse(data)?;
                Ok(CharacterServerPacket::HcNotifyZonesvr(packet))
            }
            HC_CHARACTER_LIST => {
                let packet = HcCharacterListPacket::parse(data)?;
                Ok(CharacterServerPacket::HcCharacterList(packet))
            }
            HC_ACCEPT_MAKECHAR => {
                let packet = HcAcceptMakecharPacket::parse(data)?;
                Ok(CharacterServerPacket::HcAcceptMakechar(packet))
            }
            HC_REFUSE_MAKECHAR => {
                let packet = HcRefuseMakecharPacket::parse(data)?;
                Ok(CharacterServerPacket::HcRefuseMakechar(packet))
            }
            HC_ACCEPT_DELETECHAR => {
                let packet = HcAcceptDeletecharPacket::parse(data)?;
                Ok(CharacterServerPacket::HcAcceptDeletechar(packet))
            }
            HC_REFUSE_DELETECHAR => {
                let packet = HcRefuseDeletecharPacket::parse(data)?;
                Ok(CharacterServerPacket::HcRefuseDeletechar(packet))
            }
            HC_PING => {
                let packet = HcPingPacket::parse(data)?;
                Ok(CharacterServerPacket::HcPing(packet))
            }
            HC_BLOCK_CHARACTER => {
                let packet = HcBlockCharacterPacket::parse(data)?;
                Ok(CharacterServerPacket::HcBlockCharacter(packet))
            }
            HC_SECOND_PASSWD_LOGIN => {
                let packet = HcSecondPasswdLoginPacket::parse(data)?;
                Ok(CharacterServerPacket::HcSecondPasswdLogin(packet))
            }
            HC_ACK_CHARINFO_PER_PAGE => {
                let packet = HcAckCharinfoPerPagePacket::parse(data)?;
                Ok(CharacterServerPacket::HcAckCharinfoPerPage(packet))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown character packet ID: 0x{:04X}", packet_id),
            )),
        }
    }

    fn packet_size(packet_id: u16) -> PacketSize {
        match packet_id {
            HC_ACCEPT_ENTER => PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            },
            HC_NOTIFY_ZONESVR => PacketSize::Fixed(28),
            HC_CHARACTER_LIST => PacketSize::Fixed(29),
            HC_ACCEPT_MAKECHAR => PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            },
            HC_REFUSE_MAKECHAR => PacketSize::Fixed(3),
            HC_ACCEPT_DELETECHAR => PacketSize::Fixed(2),
            HC_REFUSE_DELETECHAR => PacketSize::Fixed(3),
            HC_PING => PacketSize::Fixed(2),
            HC_BLOCK_CHARACTER => PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            },
            HC_SECOND_PASSWD_LOGIN => PacketSize::Fixed(12),
            HC_ACK_CHARINFO_PER_PAGE => PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            },
            _ => PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            }, // Unknown - assume variable-length and try to skip
        }
    }
}

/// Context maintained during character protocol processing
///
/// Tracks the character list, session data, and connection state.
#[derive(Debug, Default)]
pub struct CharacterContext {
    /// Characters available on this account
    pub characters: Vec<CharacterInfo>,

    /// Whether we received the 4-byte account ID acknowledgment
    /// (special packet sent after CH_ENTER)
    pub received_account_ack: bool,

    /// Account ID for this session
    pub account_id: Option<u32>,

    /// Login IDs from login server
    pub login_id1: Option<u32>,
    pub login_id2: Option<u32>,

    /// Account sex (0 = female, 1 = male)
    pub sex: u8,

    /// Zone server info after character selection
    pub zone_server_info: Option<ZoneServerInfo>,

    /// Selected character ID
    pub selected_character_id: Option<u32>,
}

/// Enum of all client packets for character protocol
#[derive(Debug, Clone)]
pub enum CharacterClientPacket {
    ChEnter(ChEnterPacket),
    ChSelectChar(ChSelectCharPacket),
    ChMakeChar(ChMakeCharPacket),
    ChDeleteChar(ChDeleteCharPacket),
    ChPing(ChPingPacket),
    ChCharlistReq(ChCharlistReqPacket),
}

impl ClientPacket for CharacterClientPacket {
    const PACKET_ID: u16 = 0; // Not used for enums

    fn serialize(&self) -> Bytes {
        match self {
            Self::ChEnter(p) => p.serialize(),
            Self::ChSelectChar(p) => p.serialize(),
            Self::ChMakeChar(p) => p.serialize(),
            Self::ChDeleteChar(p) => p.serialize(),
            Self::ChPing(p) => p.serialize(),
            Self::ChCharlistReq(p) => p.serialize(),
        }
    }

    fn packet_id(&self) -> u16 {
        match self {
            Self::ChEnter(_) => CH_ENTER,
            Self::ChSelectChar(_) => CH_SELECT_CHAR,
            Self::ChMakeChar(_) => CH_MAKE_CHAR,
            Self::ChDeleteChar(_) => CH_DELETE_CHAR,
            Self::ChPing(_) => CH_PING,
            Self::ChCharlistReq(_) => CH_CHARLIST_REQ,
        }
    }
}

/// Enum of all server packets for character protocol
#[derive(Debug, Clone)]
pub enum CharacterServerPacket {
    HcAcceptEnter(HcAcceptEnterPacket),
    HcNotifyZonesvr(HcNotifyZonesvrPacket),
    HcCharacterList(HcCharacterListPacket),
    HcAcceptMakechar(HcAcceptMakecharPacket),
    HcRefuseMakechar(HcRefuseMakecharPacket),
    HcAcceptDeletechar(HcAcceptDeletecharPacket),
    HcRefuseDeletechar(HcRefuseDeletecharPacket),
    HcPing(HcPingPacket),
    HcBlockCharacter(HcBlockCharacterPacket),
    HcSecondPasswdLogin(HcSecondPasswdLoginPacket),
    HcAckCharinfoPerPage(HcAckCharinfoPerPagePacket),
}

impl ServerPacket for CharacterServerPacket {
    const PACKET_ID: u16 = 0; // Not used for enums

    fn parse(_data: &[u8]) -> io::Result<Self> {
        unreachable!("Use Protocol::parse_server_packet instead")
    }

    fn packet_id(&self) -> u16 {
        match self {
            Self::HcAcceptEnter(_) => HC_ACCEPT_ENTER,
            Self::HcNotifyZonesvr(_) => HC_NOTIFY_ZONESVR,
            Self::HcCharacterList(_) => HC_CHARACTER_LIST,
            Self::HcAcceptMakechar(_) => HC_ACCEPT_MAKECHAR,
            Self::HcRefuseMakechar(_) => HC_REFUSE_MAKECHAR,
            Self::HcAcceptDeletechar(_) => HC_ACCEPT_DELETECHAR,
            Self::HcRefuseDeletechar(_) => HC_REFUSE_DELETECHAR,
            Self::HcPing(_) => HC_PING,
            Self::HcBlockCharacter(_) => HC_BLOCK_CHARACTER,
            Self::HcSecondPasswdLogin(_) => HC_SECOND_PASSWD_LOGIN,
            Self::HcAckCharinfoPerPage(_) => HC_ACK_CHARINFO_PER_PAGE,
        }
    }
}

/// Convenience methods for CharacterContext
impl CharacterContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize context with session data from login
    pub fn with_session(account_id: u32, login_id1: u32, login_id2: u32, sex: u8) -> Self {
        Self {
            account_id: Some(account_id),
            login_id1: Some(login_id1),
            login_id2: Some(login_id2),
            sex,
            ..Default::default()
        }
    }

    /// Add characters to the list
    pub fn add_characters(&mut self, chars: Vec<CharacterInfo>) {
        self.characters.extend(chars);
    }

    /// Clear the character list
    pub fn clear_characters(&mut self) {
        self.characters.clear();
    }

    /// Mark account acknowledgment as received
    pub fn acknowledge_account(&mut self) {
        self.received_account_ack = true;
    }

    /// Set zone server info after character selection
    pub fn set_zone_server(&mut self, zone_info: ZoneServerInfo) {
        self.selected_character_id = Some(zone_info.char_id);
        self.zone_server_info = Some(zone_info);
    }

    /// Get the selected character's data
    pub fn get_selected_character(&self) -> Option<&CharacterInfo> {
        let char_id = self.selected_character_id?;
        self.characters.iter().find(|c| c.char_id == char_id)
    }

    /// Reset context for new connection
    pub fn reset(&mut self) {
        self.characters.clear();
        self.received_account_ack = false;
        self.zone_server_info = None;
        self.selected_character_id = None;
    }
}
