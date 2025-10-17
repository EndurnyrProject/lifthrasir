use serde::{Deserialize, Serialize};
use std::io::{self, Cursor, Read};
use byteorder::{LittleEndian, ReadBytesExt};

/// Character information structure
///
/// Contains all data about a character including stats, appearance,
/// equipment, and progression. This structure is used across multiple
/// packets (HC_ACCEPT_ENTER, HC_ACCEPT_MAKECHAR, HC_ACK_CHARINFO_PER_PAGE).
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
    /// Size of a character info structure in bytes (175 bytes for HC_ACCEPT_ENTER)
    ///
    /// Structure breakdown:
    /// - 12x u32 (48 bytes): char_id, zeny, job_level, body_state, health_state, option,
    ///                       karma, manner, delete_date, robe, char_slot_change, char_rename
    /// - 6x u64 (48 bytes): base_exp, job_exp, hp, max_hp, sp, max_sp
    /// - 15x u16 (30 bytes): status_point, walk_speed, class, hair, body, weapon, base_level,
    ///                       skill_point, head_bottom, shield, head_top, head_mid, hair_color,
    ///                       clothes_color, rename
    /// - 9x u8 (9 bytes): str, agi, vit, int, dex, luk, char_num, hair_color_alt, sex
    /// - name: [u8; 24] (24 bytes)
    /// - last_map: [u8; 16] (16 bytes)
    /// Total: 175 bytes
    pub const SIZE_ACCEPT_ENTER: usize = 175;

    /// Size of a character info structure in bytes (175 bytes for HC_ACK_CHARINFO_PER_PAGE)
    pub const SIZE_CHARINFO_PER_PAGE: usize = 175;

    /// Parse a character from HC_ACCEPT_ENTER or HC_ACCEPT_MAKECHAR packet data
    ///
    /// This uses the 175-byte structure format.
    pub fn parse(data: &[u8]) -> io::Result<Self> {
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
        let _hair_color_alt = cursor.read_u8()?; // Duplicate hair color field

        let rename_u16 = cursor.read_u16::<LittleEndian>()?;
        let rename = rename_u16 as u8;

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

/// Blocked character entry for HC_BLOCK_CHARACTER packet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedCharacterEntry {
    pub char_id: u32,
    pub expire_date: String,
}

impl BlockedCharacterEntry {
    /// Size of a blocked character entry in bytes (24 bytes)
    pub const SIZE: usize = 24;

    /// Parse a blocked character entry from packet data
    pub fn parse(cursor: &mut Cursor<&[u8]>) -> io::Result<Self> {
        let char_id = cursor.read_u32::<LittleEndian>()?;

        let mut date_bytes = [0u8; 20];
        cursor.read_exact(&mut date_bytes)?;
        let expire_date = String::from_utf8_lossy(&date_bytes)
            .trim_end_matches('\0')
            .to_string();

        Ok(Self {
            char_id,
            expire_date,
        })
    }
}

/// Zone server connection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneServerInfo {
    pub char_id: u32,
    pub map_name: String,
    pub ip: [u8; 4],
    pub port: u16,
}

impl ZoneServerInfo {
    /// Convert IP address to dotted decimal notation
    pub fn ip_string(&self) -> String {
        format!("{}.{}.{}.{}", self.ip[0], self.ip[1], self.ip[2], self.ip[3])
    }
}

/// Character slot information from HC_CHARACTER_LIST packet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterSlotInfo {
    pub normal_slots: u8,
    pub premium_slots: u8,
    pub billing_slots: u8,
    pub producible_slots: u8,
    pub valid_slots: u8,
}

/// Second password login state
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SecondPasswordState {
    /// Pincode disabled or correct
    Disabled,
    /// Ask for pincode
    AskForPincode,
    /// Create new pincode
    CreateNew,
    /// Pincode must be changed
    MustChange,
    /// Create new pincode (duplicate state)
    CreateNewAlt,
    /// System message 1896
    SystemMessage,
    /// Unable to use KSSN number
    KssnError,
    /// Show button for pincode
    ShowButton,
    /// Pincode was incorrect
    Incorrect,
    /// Unknown state
    Unknown(u16),
}

impl From<u16> for SecondPasswordState {
    fn from(value: u16) -> Self {
        match value {
            0 => SecondPasswordState::Disabled,
            1 => SecondPasswordState::AskForPincode,
            2 => SecondPasswordState::CreateNew,
            3 => SecondPasswordState::MustChange,
            4 => SecondPasswordState::CreateNewAlt,
            5 => SecondPasswordState::SystemMessage,
            6 => SecondPasswordState::KssnError,
            7 => SecondPasswordState::ShowButton,
            8 => SecondPasswordState::Incorrect,
            other => SecondPasswordState::Unknown(other),
        }
    }
}

impl SecondPasswordState {
    pub fn as_u16(&self) -> u16 {
        match self {
            SecondPasswordState::Disabled => 0,
            SecondPasswordState::AskForPincode => 1,
            SecondPasswordState::CreateNew => 2,
            SecondPasswordState::MustChange => 3,
            SecondPasswordState::CreateNewAlt => 4,
            SecondPasswordState::SystemMessage => 5,
            SecondPasswordState::KssnError => 6,
            SecondPasswordState::ShowButton => 7,
            SecondPasswordState::Incorrect => 8,
            SecondPasswordState::Unknown(value) => *value,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SecondPasswordState::Disabled => "Pincode disabled or correct",
            SecondPasswordState::AskForPincode => "Ask for pincode",
            SecondPasswordState::CreateNew => "Create new pincode",
            SecondPasswordState::MustChange => "Pincode must be changed",
            SecondPasswordState::CreateNewAlt => "Create new pincode",
            SecondPasswordState::SystemMessage => "System message 1896",
            SecondPasswordState::KssnError => "Unable to use KSSN number",
            SecondPasswordState::ShowButton => "Show button for pincode",
            SecondPasswordState::Incorrect => "Pincode was incorrect",
            SecondPasswordState::Unknown(_) => "Unknown state",
        }
    }
}

/// Character creation error codes
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CharCreationError {
    /// Character name already exists
    NameExists,
    /// Character name contains invalid characters
    InvalidName,
    /// Other error
    Unknown(u8),
}

impl From<u8> for CharCreationError {
    fn from(value: u8) -> Self {
        match value {
            0x00 => CharCreationError::NameExists,
            0xFF => CharCreationError::InvalidName,
            other => CharCreationError::Unknown(other),
        }
    }
}

/// Character deletion error codes
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CharDeletionError {
    /// Not eligible to delete
    NotEligible,
    /// Other error
    Unknown(u8),
}

impl From<u8> for CharDeletionError {
    fn from(value: u8) -> Self {
        match value {
            0x00 => CharDeletionError::NotEligible,
            other => CharDeletionError::Unknown(other),
        }
    }
}
