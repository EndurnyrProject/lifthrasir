pub mod client_packets;
pub mod handlers;
pub mod protocol;
pub mod server_packets;
pub mod types;

// Re-export commonly used types
pub use client_packets::{
    ChCharlistReqPacket, ChEnterPacket, ChMakeCharPacket, ChPingPacket, ChReqCharDelete2Packet,
    ChSelectCharPacket, CH_CHARLIST_REQ, CH_ENTER, CH_MAKE_CHAR, CH_PING, CH_REQ_CHAR_DELETE2,
    CH_SELECT_CHAR,
};

pub use handlers::{
    AcceptEnterHandler, AcceptMakecharHandler, AckCharinfoPerPageHandler, BlockCharacterHandler,
    BlockedCharactersReceived, CharDelete2AckHandler, CharacterCreated, CharacterCreationFailed,
    CharacterDeleted, CharacterDeletionFailed, CharacterInfoPageReceived, CharacterListHandler,
    CharacterServerConnected, CharacterSlotInfoReceived, CharlistNotifyHandler,
    NotifyZonesvrHandler, PingHandler, PingReceived, RefuseMakecharHandler,
    SecondPasswdLoginHandler, SecondPasswordRequested, ZoneServerInfoReceived,
};

pub use protocol::{
    CharacterClientPacket, CharacterContext, CharacterProtocol, CharacterServerPacket,
};

pub use server_packets::{
    HcAcceptEnterPacket, HcAcceptMakecharPacket, HcAckCharinfoPerPagePacket,
    HcBlockCharacterPacket, HcCharDelete2AckPacket, HcCharacterListPacket, HcCharlistNotifyPacket,
    HcNotifyZonesvrPacket, HcPingPacket, HcRefuseMakecharPacket, HcSecondPasswdLoginPacket,
    HC_ACCEPT_ENTER, HC_ACCEPT_MAKECHAR, HC_ACK_CHARINFO_PER_PAGE, HC_BLOCK_CHARACTER,
    HC_CHARACTER_LIST, HC_CHARLIST_NOTIFY, HC_CHAR_DELETE2_ACK, HC_NOTIFY_ZONESVR, HC_PING,
    HC_REFUSE_MAKECHAR, HC_SECOND_PASSWD_LOGIN,
};

pub use types::{
    BlockedCharacterEntry, CharCreationError, CharDeletionError, CharacterInfo, CharacterSlotInfo,
    SecondPasswordState, ZoneServerInfo,
};
