use bevy::log::{debug, error, info, warn};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use std::io::{self, Cursor, Read, Write};

// Packet IDs for character server communication
pub const CH_ENTER: u16 = 0x0065;
pub const HC_ACCEPT_ENTER: u16 = 0x006B;
pub const CH_SELECT_CHAR: u16 = 0x0066;
pub const HC_NOTIFY_ZONESVR: u16 = 0x0071;
pub const CH_MAKE_CHAR: u16 = 0x0A39;
pub const HC_ACCEPT_MAKECHAR: u16 = 0x0B6F;
pub const CH_DELETE_CHAR: u16 = 0x0068;
pub const HC_REFUSE_MAKECHAR: u16 = 0x006E;
pub const HC_ACCEPT_DELETECHAR: u16 = 0x006F;
pub const HC_REFUSE_DELETECHAR: u16 = 0x0070;
pub const CH_PING: u16 = 0x0187;
pub const HC_PING: u16 = 0x0187;
pub const CH_CHARLIST_REQ: u16 = 0x09A1;
pub const HC_ACK_CHARINFO_PER_PAGE: u16 = 0x099D;
pub const HC_CHARACTER_LIST: u16 = 0x082D;
pub const HC_BLOCK_CHARACTER: u16 = 0x020D;
pub const HC_SECOND_PASSWD_LOGIN: u16 = 0x08B9;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterInfo {
    pub char_id: u32,
    pub base_exp: u64,
    pub zeny: u32,
    pub job_exp: u64,
    pub job_level: u32,
    pub body_state: u32,
    pub health_state: u32,
    pub option: u32,
    pub karma: u32,
    pub manner: u32,
    pub status_point: u16,
    pub hp: u64,
    pub max_hp: u64,
    pub sp: u64,
    pub max_sp: u64,
    pub walk_speed: u16,
    pub class: u16,
    pub hair: u16,
    pub body: u16,
    pub weapon: u16,
    pub base_level: u16,
    pub skill_point: u16,
    pub head_bottom: u16,
    pub shield: u16,
    pub head_top: u16,
    pub head_mid: u16,
    pub hair_color: u16,
    pub clothes_color: u16,
    pub name: String,
    pub str: u8,
    pub agi: u8,
    pub vit: u8,
    pub int: u8,
    pub dex: u8,
    pub luk: u8,
    pub char_num: u8,
    pub rename: u8,
    pub last_map: String,
    pub delete_date: u32,
    pub robe: u32,
    pub char_slot_change: u32,
    pub char_rename: u32,
    pub sex: u8,
}

impl CharacterInfo {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        // This method parses a single character from HC_ACCEPT_MAKECHAR packet
        // The character data in this packet has the same structure as in HC_ACCEPT_ENTER
        let mut cursor = Cursor::new(data);

        let char_id = cursor.read_u32::<LittleEndian>()?;
        let base_exp = cursor.read_u64::<LittleEndian>()?;
        let zeny = cursor.read_u32::<LittleEndian>()?;
        let job_exp = cursor.read_u64::<LittleEndian>()?;
        let job_level = cursor.read_u32::<LittleEndian>()?;
        let body_state = cursor.read_u32::<LittleEndian>()?;
        let health_state = cursor.read_u32::<LittleEndian>()?;
        let option = cursor.read_u32::<LittleEndian>()?;
        let karma = cursor.read_u32::<LittleEndian>()?;
        let manner = cursor.read_u32::<LittleEndian>()?;
        let status_point = cursor.read_u16::<LittleEndian>()?;
        let hp = cursor.read_u64::<LittleEndian>()?;
        let max_hp = cursor.read_u64::<LittleEndian>()?;
        let sp = cursor.read_u64::<LittleEndian>()?;
        let max_sp = cursor.read_u64::<LittleEndian>()?;
        let walk_speed = cursor.read_u16::<LittleEndian>()?;
        let class = cursor.read_u16::<LittleEndian>()?;
        let hair = cursor.read_u16::<LittleEndian>()?;
        let body = cursor.read_u16::<LittleEndian>()?;
        let weapon = cursor.read_u16::<LittleEndian>()?;
        let base_level = cursor.read_u16::<LittleEndian>()?;
        let skill_point = cursor.read_u16::<LittleEndian>()?;
        let head_bottom = cursor.read_u16::<LittleEndian>()?;
        let shield = cursor.read_u16::<LittleEndian>()?;
        let head_top = cursor.read_u16::<LittleEndian>()?;
        let head_mid = cursor.read_u16::<LittleEndian>()?;
        let hair_color = cursor.read_u16::<LittleEndian>()?;
        let clothes_color = cursor.read_u16::<LittleEndian>()?;

        // Read character name (24 bytes)
        let mut name_bytes = [0u8; 24];
        cursor.read_exact(&mut name_bytes)?;
        let name = String::from_utf8_lossy(&name_bytes)
            .trim_end_matches('\0')
            .to_string();

        let str = cursor.read_u8()?;
        let agi = cursor.read_u8()?;
        let vit = cursor.read_u8()?;
        let int = cursor.read_u8()?;
        let dex = cursor.read_u8()?;
        let luk = cursor.read_u8()?;
        let char_num = cursor.read_u8()?;
        let _hair_color_alt = cursor.read_u8()?; // Second hair color field (duplicate)

        let rename_u16 = cursor.read_u16::<LittleEndian>()?; // rename is u16 not u8
        let rename = rename_u16 as u8; // Convert to u8 for our structure

        // Read last map (16 bytes)
        let mut map_bytes = [0u8; 16];
        cursor.read_exact(&mut map_bytes)?;
        let last_map = String::from_utf8_lossy(&map_bytes)
            .trim_end_matches('\0')
            .to_string();

        let delete_date = cursor.read_u32::<LittleEndian>()?;
        let robe = cursor.read_u32::<LittleEndian>()?;
        let char_slot_change = cursor.read_u32::<LittleEndian>()?;
        let char_rename = cursor.read_u32::<LittleEndian>()?;
        let sex = cursor.read_u8()?;

        Ok(CharacterInfo {
            char_id,
            base_exp,
            zeny,
            job_exp,
            job_level,
            body_state,
            health_state,
            option,
            karma,
            manner,
            status_point,
            hp,
            max_hp,
            sp,
            max_sp,
            walk_speed,
            class,
            hair,
            body,
            weapon,
            base_level,
            skill_point,
            head_bottom,
            shield,
            head_top,
            head_mid,
            hair_color,
            clothes_color,
            name,
            str,
            agi,
            vit,
            int,
            dex,
            luk,
            char_num,
            rename,
            last_map,
            delete_date,
            robe,
            char_slot_change,
            char_rename,
            sex,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ChEnterPacket {
    pub account_id: u32,
    pub login_id1: u32,
    pub login_id2: u32,
    pub unknown: u16,
    pub sex: u8,
}

impl ChEnterPacket {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.write_u16::<LittleEndian>(CH_ENTER).unwrap();
        buf.write_u32::<LittleEndian>(self.account_id).unwrap();
        buf.write_u32::<LittleEndian>(self.login_id1).unwrap();
        buf.write_u32::<LittleEndian>(self.login_id2).unwrap();
        buf.write_u16::<LittleEndian>(self.unknown).unwrap();
        buf.write_u8(self.sex).unwrap();
        buf
    }
}

#[derive(Debug, Clone)]
pub struct HcAcceptEnterPacket {
    pub packet_len: u16,
    pub max_chars: u8,
    pub available_slots: u8,
    pub premium_slots: u8,
    pub characters: Vec<CharacterInfo>,
}

impl HcAcceptEnterPacket {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(data);

        let _packet_id = cursor.read_u16::<LittleEndian>()?;
        let packet_len = cursor.read_u16::<LittleEndian>()?;

        // Read the actual header structure from server
        let max_chars = cursor.read_u8()?; // Max slots (15)
        let available_slots = cursor.read_u8()?; // Available slots (9)
        let premium_slots = cursor.read_u8()?; // Premium slots (9)

        // Skip 20 unknown bytes
        for _ in 0..20 {
            cursor.read_u8()?;
        }

        let mut characters = Vec::new();

        // Calculate character data size
        // Header: 4 (id+len) + 3 (slots) + 20 (unknown) = 27 bytes
        let char_data_size = packet_len as usize - 27;

        // Calculate character size based on actual server structure
        const CHARACTER_DATA_SIZE: usize = 155; // Corrected size based on server

        if char_data_size > 0 {
            let char_count = char_data_size / CHARACTER_DATA_SIZE;
            debug!(
                "Parsing {} characters (data_size: {}, char_size: {})",
                char_count, char_data_size, CHARACTER_DATA_SIZE
            );

            if char_data_size % CHARACTER_DATA_SIZE != 0 {
                warn!(
                    "Character data size {} is not a multiple of {} - possible version mismatch",
                    char_data_size, CHARACTER_DATA_SIZE
                );
            }

            for i in 0..char_count {
                match Self::parse_character(&mut cursor) {
                    Ok(char_info) => {
                        debug!("Successfully parsed character {}: {}", i, char_info.name);
                        characters.push(char_info);
                    }
                    Err(e) => {
                        error!("Failed to parse character {}: {:?}", i, e);
                        break;
                    }
                }
            }
        }

        info!(
            "Successfully parsed HC_ACCEPT_ENTER with {} characters",
            characters.len()
        );

        Ok(Self {
            packet_len,
            max_chars,
            available_slots,
            premium_slots,
            characters,
        })
    }

    fn parse_character(cursor: &mut Cursor<&[u8]>) -> io::Result<CharacterInfo> {
        let char_id = cursor.read_u32::<LittleEndian>()?;
        let base_exp = cursor.read_u64::<LittleEndian>()?;
        let zeny = cursor.read_u32::<LittleEndian>()?;
        let job_exp = cursor.read_u64::<LittleEndian>()?;
        let job_level = cursor.read_u32::<LittleEndian>()?;

        // opt1 (bodystate) and opt2 (healthstate) - not used in our CharacterInfo
        let body_state = cursor.read_u32::<LittleEndian>()?;
        let health_state = cursor.read_u32::<LittleEndian>()?;

        let option = cursor.read_u32::<LittleEndian>()?;
        let karma = cursor.read_u32::<LittleEndian>()?;
        let manner = cursor.read_u32::<LittleEndian>()?;
        let status_point = cursor.read_u16::<LittleEndian>()?;

        let hp = cursor.read_u64::<LittleEndian>()?;
        let max_hp = cursor.read_u64::<LittleEndian>()?;
        let sp = cursor.read_u64::<LittleEndian>()?;
        let max_sp = cursor.read_u64::<LittleEndian>()?;

        let walk_speed = cursor.read_u16::<LittleEndian>()?;
        let class = cursor.read_u16::<LittleEndian>()?;
        let hair = cursor.read_u16::<LittleEndian>()?;
        let body = cursor.read_u16::<LittleEndian>()?; // body field for newer clients
        let weapon = cursor.read_u16::<LittleEndian>()?;
        let base_level = cursor.read_u16::<LittleEndian>()?;
        let skill_point = cursor.read_u16::<LittleEndian>()?;
        let head_bottom = cursor.read_u16::<LittleEndian>()?;
        let shield = cursor.read_u16::<LittleEndian>()?;
        let head_top = cursor.read_u16::<LittleEndian>()?;
        let head_mid = cursor.read_u16::<LittleEndian>()?;
        let hair_color = cursor.read_u16::<LittleEndian>()?;
        let clothes_color = cursor.read_u16::<LittleEndian>()?;

        // Read character name (24 bytes)
        let mut name_bytes = [0u8; 24];
        cursor.read_exact(&mut name_bytes)?;
        let name = String::from_utf8_lossy(&name_bytes)
            .trim_end_matches('\0')
            .to_string();

        let str = cursor.read_u8()?;
        let agi = cursor.read_u8()?;
        let vit = cursor.read_u8()?;
        let int = cursor.read_u8()?;
        let dex = cursor.read_u8()?;
        let luk = cursor.read_u8()?;
        let char_num = cursor.read_u8()?;
        let _hair_color_alt = cursor.read_u8()?; // Second hair color field (duplicate)

        let rename_u16 = cursor.read_u16::<LittleEndian>()?; // rename is u16 not u8
        let rename = rename_u16 as u8; // Convert to u8 for our structure

        // Read map name (16 bytes) - last_map
        let mut map_bytes = [0u8; 16];
        cursor.read_exact(&mut map_bytes)?;
        let last_map = String::from_utf8_lossy(&map_bytes)
            .trim_end_matches('\0')
            .to_string();

        let delete_date = cursor.read_u32::<LittleEndian>()?;
        let robe = cursor.read_u32::<LittleEndian>()?;
        let char_slot_change = cursor.read_u32::<LittleEndian>()?;
        let char_rename = cursor.read_u32::<LittleEndian>()?;
        let sex = cursor.read_u8()?;

        Ok(CharacterInfo {
            char_id,
            base_exp,
            zeny,
            job_exp,
            job_level,
            body_state,
            health_state,
            option,
            karma,
            manner,
            status_point,
            hp,
            max_hp,
            sp,
            max_sp,
            walk_speed,
            class,
            hair,
            body,
            weapon,
            base_level,
            skill_point,
            head_bottom,
            shield,
            head_top,
            head_mid,
            hair_color,
            clothes_color,
            name,
            str,
            agi,
            vit,
            int,
            dex,
            luk,
            char_num,
            rename,
            last_map,
            delete_date,
            robe,
            char_slot_change,
            char_rename,
            sex,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ChSelectCharPacket {
    pub char_num: u8,
}

impl ChSelectCharPacket {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.write_u16::<LittleEndian>(CH_SELECT_CHAR).unwrap();
        buf.write_u8(self.char_num).unwrap();
        buf
    }
}

#[derive(Debug, Clone)]
pub struct HcNotifyZonesvrPacket {
    pub char_id: u32,
    pub map_name: String,
    pub ip: [u8; 4],
    pub port: u16,
}

impl HcNotifyZonesvrPacket {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(data);

        let _packet_id = cursor.read_u16::<LittleEndian>()?;
        let char_id = cursor.read_u32::<LittleEndian>()?;

        // Read map name (16 bytes)
        let mut map_bytes = [0u8; 16];
        cursor.read_exact(&mut map_bytes)?;
        let map_name = String::from_utf8_lossy(&map_bytes)
            .trim_end_matches('\0')
            .to_string();

        let mut ip = [0u8; 4];
        cursor.read_exact(&mut ip)?;
        let port = cursor.read_u16::<LittleEndian>()?;

        Ok(Self {
            char_id,
            map_name,
            ip,
            port,
        })
    }
}

#[derive(Debug, Clone)]
pub struct HcCharacterListPacket {
    pub normal_slots: u8,
    pub premium_slots: u8,
    pub billing_slots: u8,
    pub producible_slots: u8,
    pub valid_slots: u8,
}

impl HcCharacterListPacket {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(data);

        let _packet_id = cursor.read_u16::<LittleEndian>()?;
        let _packet_len = cursor.read_u16::<LittleEndian>()?; // Should be 29

        let normal_slots = cursor.read_u8()?;
        let premium_slots = cursor.read_u8()?;
        let billing_slots = cursor.read_u8()?;
        let producible_slots = cursor.read_u8()?;
        let valid_slots = cursor.read_u8()?;

        // Skip 20 unused bytes
        for _ in 0..20 {
            cursor.read_u8()?;
        }

        Ok(Self {
            normal_slots,
            premium_slots,
            billing_slots,
            producible_slots,
            valid_slots,
        })
    }
}

#[derive(Debug, Clone)]
pub struct BlockedCharacterEntry {
    pub char_id: u32,
    pub expire_date: String,
}

#[derive(Debug, Clone)]
pub struct HcBlockCharacterPacket {
    pub blocked_chars: Vec<BlockedCharacterEntry>,
}

impl HcBlockCharacterPacket {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(data);

        let _packet_id = cursor.read_u16::<LittleEndian>()?;
        let packet_len = cursor.read_u16::<LittleEndian>()?;

        let mut blocked_chars = Vec::new();

        // Each entry is 24 bytes (4 bytes char_id + 20 bytes expire_date)
        let entry_count = (packet_len as usize - 4) / 24;

        for _ in 0..entry_count {
            let char_id = cursor.read_u32::<LittleEndian>()?;

            let mut date_bytes = [0u8; 20];
            cursor.read_exact(&mut date_bytes)?;
            let expire_date = String::from_utf8_lossy(&date_bytes)
                .trim_end_matches('\0')
                .to_string();

            blocked_chars.push(BlockedCharacterEntry {
                char_id,
                expire_date,
            });
        }

        Ok(Self { blocked_chars })
    }
}

#[derive(Debug, Clone)]
pub struct HcSecondPasswdLoginPacket {
    pub seed: u32,
    pub account_id: u32,
    pub state: u16,
}

impl HcSecondPasswdLoginPacket {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(data);

        let _packet_id = cursor.read_u16::<LittleEndian>()?;
        let seed = cursor.read_u32::<LittleEndian>()?;
        let account_id = cursor.read_u32::<LittleEndian>()?;
        let state = cursor.read_u16::<LittleEndian>()?;

        Ok(Self {
            seed,
            account_id,
            state,
        })
    }

    pub fn state_description(&self) -> &'static str {
        match self.state {
            0 => "Pincode disabled or correct",
            1 => "Ask for pincode",
            2 => "Create new pincode",
            3 => "Pincode must be changed",
            4 => "Create new pincode",
            5 => "System message 1896",
            6 => "Unable to use KSSN number",
            7 => "Show button for pincode",
            8 => "Pincode was incorrect",
            _ => "Unknown state",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChMakeCharPacket {
    pub name: String,
    pub slot: u8,
    pub hair_color: u16,
    pub hair_style: u16,
    pub starting_job: u16,
    pub sex: u8,
}

impl ChMakeCharPacket {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.write_u16::<LittleEndian>(CH_MAKE_CHAR).unwrap();

        // Write character name (24 bytes, null-terminated)
        let mut name_bytes = [0u8; 24];
        let name_data = self.name.as_bytes();
        let len = name_data.len().min(23);
        name_bytes[..len].copy_from_slice(&name_data[..len]);
        buf.write_all(&name_bytes).unwrap();

        buf.write_u8(self.slot).unwrap();
        buf.write_u16::<LittleEndian>(self.hair_color).unwrap();
        buf.write_u16::<LittleEndian>(self.hair_style).unwrap();
        buf.write_u16::<LittleEndian>(self.starting_job).unwrap();
        buf.write_u16::<LittleEndian>(0).unwrap(); // unknown
        buf.write_u8(self.sex).unwrap();

        buf
    }
}

#[derive(Debug, Clone)]
pub struct ChDeleteCharPacket {
    pub char_id: u32,
    pub email: String,
}

impl ChDeleteCharPacket {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.write_u16::<LittleEndian>(CH_DELETE_CHAR).unwrap();
        buf.write_u32::<LittleEndian>(self.char_id).unwrap();

        // Write email (50 bytes)
        let mut email_bytes = [0u8; 50];
        let email_data = self.email.as_bytes();
        let len = email_data.len().min(49);
        email_bytes[..len].copy_from_slice(&email_data[..len]);
        buf.write_all(&email_bytes).unwrap();

        buf
    }
}

#[derive(Debug, Clone)]
pub struct ChPingPacket;

impl ChPingPacket {
    pub fn serialize() -> Vec<u8> {
        let mut buf = Vec::new();
        buf.write_u16::<LittleEndian>(CH_PING).unwrap();
        buf.write_u32::<LittleEndian>(0).unwrap(); // account_id placeholder
        buf
    }
}

#[derive(Debug, Clone)]
pub struct ChCharlistReqPacket;

impl ChCharlistReqPacket {
    pub fn serialize() -> Vec<u8> {
        let mut buf = Vec::new();
        buf.write_u16::<LittleEndian>(CH_CHARLIST_REQ).unwrap();
        buf
    }
}

#[derive(Debug, Clone)]
pub struct HcAckCharinfoPerPagePacket {
    pub characters: Vec<CharacterInfo>,
}

impl HcAckCharinfoPerPagePacket {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(data);

        let _packet_id = cursor.read_u16::<LittleEndian>()?;
        let packet_len = cursor.read_u16::<LittleEndian>()?;

        if packet_len < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid packet length",
            ));
        }

        let data_len = (packet_len - 4) as usize;
        const CHAR_INFO_SIZE: usize = 175; // Size of each character entry

        if data_len % CHAR_INFO_SIZE != 0 {
            warn!(
                "HC_ACK_CHARINFO_PER_PAGE data length {} not multiple of {}",
                data_len, CHAR_INFO_SIZE
            );
        }

        let char_count = data_len / CHAR_INFO_SIZE;
        let mut characters = Vec::with_capacity(char_count);

        for _ in 0..char_count {
            let mut char_data = vec![0u8; CHAR_INFO_SIZE];
            cursor.read_exact(&mut char_data)?;
            match CharacterInfo::parse(&char_data) {
                Ok(char_info) => characters.push(char_info),
                Err(e) => {
                    error!("Failed to parse character info: {}", e);
                    continue;
                }
            }
        }

        Ok(Self { characters })
    }
}

#[derive(Debug, Clone)]
pub enum CharServerPacket {
    ChEnter(ChEnterPacket),
    ChSelectChar(ChSelectCharPacket),
    ChMakeChar(ChMakeCharPacket),
    ChDeleteChar(ChDeleteCharPacket),
    ChPing,
    ChCharlistReq,
}

#[derive(Debug, Clone)]
pub enum CharServerResponse {
    HcAcceptEnter(HcAcceptEnterPacket),
    HcNotifyZonesvr(HcNotifyZonesvrPacket),
    HcAcceptMakeChar(CharacterInfo),
    HcAcceptDeleteChar,
    HcRefuseMakeChar(u8),   // error code
    HcRefuseDeleteChar(u8), // error code
    HcPing,
    HcCharacterList(HcCharacterListPacket),
    HcAckCharinfoPerPage(HcAckCharinfoPerPagePacket),
    HcBlockCharacter(HcBlockCharacterPacket),
    HcSecondPasswdLogin(HcSecondPasswdLoginPacket),
}
