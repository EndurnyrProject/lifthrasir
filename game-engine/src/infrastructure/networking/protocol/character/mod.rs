pub mod client_packets;
pub mod handlers;
pub mod protocol;
pub mod server_packets;
pub mod types;

// Re-export commonly used types
pub use client_packets::{
    ChCharlistReqPacket, ChDeleteCharPacket, ChEnterPacket, ChMakeCharPacket, ChPingPacket,
    ChSelectCharPacket, CH_CHARLIST_REQ, CH_DELETE_CHAR, CH_ENTER, CH_MAKE_CHAR, CH_PING,
    CH_SELECT_CHAR,
};

pub use handlers::{
    AcceptDeletecharHandler, AcceptEnterHandler, AcceptMakecharHandler,
    AckCharinfoPerPageHandler, BlockCharacterHandler, CharacterCreated, CharacterCreationFailed,
    CharacterDeleted, CharacterDeletionFailed, CharacterInfoPageReceived, CharacterListHandler,
    CharacterServerConnected, CharacterSlotInfoReceived, BlockedCharactersReceived, NotifyZonesvrHandler,
    PingHandler, PingReceived, RefuseDeletecharHandler, RefuseMakecharHandler,
    SecondPasswdLoginHandler, SecondPasswordRequested, ZoneServerInfoReceived,
};

pub use protocol::{
    CharacterClientPacket, CharacterContext, CharacterProtocol, CharacterServerPacket,
};

pub use server_packets::{
    HcAcceptDeletecharPacket, HcAcceptEnterPacket, HcAcceptMakecharPacket,
    HcAckCharinfoPerPagePacket, HcBlockCharacterPacket, HcCharacterListPacket,
    HcNotifyZonesvrPacket, HcPingPacket, HcRefuseDeletecharPacket, HcRefuseMakecharPacket,
    HcSecondPasswdLoginPacket, HC_ACCEPT_DELETECHAR, HC_ACCEPT_ENTER, HC_ACCEPT_MAKECHAR,
    HC_ACK_CHARINFO_PER_PAGE, HC_BLOCK_CHARACTER, HC_CHARACTER_LIST, HC_NOTIFY_ZONESVR, HC_PING,
    HC_REFUSE_DELETECHAR, HC_REFUSE_MAKECHAR, HC_SECOND_PASSWD_LOGIN,
};

pub use types::{
    BlockedCharacterEntry, CharCreationError, CharDeletionError, CharacterInfo,
    CharacterSlotInfo, SecondPasswordState, ZoneServerInfo,
};
