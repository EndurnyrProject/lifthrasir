use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Component)]
pub struct Character {
    pub info: CharacterData,
    pub slot: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterData {
    pub char_id: u32,
    pub name: String,
    pub class: JobClass,
    pub base_level: u16,
    pub job_level: u16,
    pub base_exp: u64,
    pub job_exp: u64,
    pub hp: u64,
    pub max_hp: u64,
    pub sp: u64,
    pub max_sp: u64,
    pub zeny: u32,
    pub str: u8,
    pub agi: u8,
    pub vit: u8,
    pub int: u8,
    pub dex: u8,
    pub luk: u8,
    pub hair_style: u16,
    pub hair_color: u16,
    pub clothes_color: u16,
    pub weapon: u16,
    pub shield: u16,
    pub head_top: u16,
    pub head_mid: u16,
    pub head_bottom: u16,
    pub robe: u32,
    pub last_map: String,
    pub delete_date: Option<u32>,
    pub sex: Gender,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Gender {
    Female = 0,
    Male = 1,
}

impl From<u8> for Gender {
    fn from(value: u8) -> Self {
        match value {
            0 => Gender::Female,
            _ => Gender::Male,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u16)]
pub enum JobClass {
    Novice = 0,
    Swordsman = 1,
    Magician = 2,
    Archer = 3,
    Acolyte = 4,
    Merchant = 5,
    Thief = 6,
    Knight = 7,
    Priest = 8,
    Wizard = 9,
    Blacksmith = 10,
    Hunter = 11,
    Assassin = 12,
    Crusader = 14,
    Monk = 15,
    Sage = 16,
    Rogue = 17,
    Alchemist = 18,
    BardDancer = 19,
}

impl From<u16> for JobClass {
    fn from(value: u16) -> Self {
        match value {
            0 => JobClass::Novice,
            1 => JobClass::Swordsman,
            2 => JobClass::Magician,
            3 => JobClass::Archer,
            4 => JobClass::Acolyte,
            5 => JobClass::Merchant,
            6 => JobClass::Thief,
            7 => JobClass::Knight,
            8 => JobClass::Priest,
            9 => JobClass::Wizard,
            10 => JobClass::Blacksmith,
            11 => JobClass::Hunter,
            12 => JobClass::Assassin,
            14 => JobClass::Crusader,
            15 => JobClass::Monk,
            16 => JobClass::Sage,
            17 => JobClass::Rogue,
            18 => JobClass::Alchemist,
            19 => JobClass::BardDancer,
            _ => JobClass::Novice, // Default to Novice for unknown classes
        }
    }
}

impl JobClass {
    /// Convert job class to Korean sprite name for file paths
    pub fn to_sprite_name(&self) -> &str {
        match self {
            JobClass::Novice => "초보자",
            JobClass::Swordsman => "검사",
            JobClass::Magician => "마법사",
            JobClass::Archer => "궁수",
            JobClass::Acolyte => "성직자",
            JobClass::Merchant => "상인",
            JobClass::Thief => "도둑",
            JobClass::Knight => "기사",
            JobClass::Priest => "프리스트",
            JobClass::Wizard => "위저드",
            JobClass::Blacksmith => "제철공",
            JobClass::Hunter => "헌터",
            JobClass::Assassin => "어세신",
            JobClass::Crusader => "크루세이더",
            JobClass::Monk => "몽크",
            JobClass::Sage => "세이지",
            JobClass::Rogue => "로그",
            JobClass::Alchemist => "알케미스트",
            JobClass::BardDancer => "바드댄서",
        }
    }
    pub fn to_display_name(&self) -> &'static str {
        match self {
            JobClass::Novice => "Novice",
            JobClass::Swordsman => "Swordsman",
            JobClass::Magician => "Magician",
            JobClass::Archer => "Archer",
            JobClass::Acolyte => "Acolyte",
            JobClass::Merchant => "Merchant",
            JobClass::Thief => "Thief",
            JobClass::Knight => "Knight",
            JobClass::Priest => "Priest",
            JobClass::Wizard => "Wizard",
            JobClass::Blacksmith => "Blacksmith",
            JobClass::Hunter => "Hunter",
            JobClass::Assassin => "Assassin",
            JobClass::Crusader => "Crusader",
            JobClass::Monk => "Monk",
            JobClass::Sage => "Sage",
            JobClass::Rogue => "Rogue",
            JobClass::Alchemist => "Alchemist",
            JobClass::BardDancer => "Bard/Dancer",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterCreationForm {
    pub name: String,
    pub slot: u8,
    pub hair_style: u16,
    pub hair_color: u16,
    pub starting_job: JobClass,
    pub sex: Gender,
    pub str: u8,
    pub agi: u8,
    pub vit: u8,
    pub int: u8,
    pub dex: u8,
    pub luk: u8,
}

impl Default for CharacterCreationForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            slot: 0,
            hair_style: 1,
            hair_color: 0,
            starting_job: JobClass::Novice,
            sex: Gender::Male,
            str: 5,
            agi: 5,
            vit: 5,
            int: 5,
            dex: 5,
            luk: 5,
        }
    }
}

impl CharacterCreationForm {
    pub fn validate(&self) -> Result<(), CharacterCreationError> {
        // Name validation
        if self.name.is_empty() {
            return Err(CharacterCreationError::NameEmpty);
        }
        if self.name.len() < 4 {
            return Err(CharacterCreationError::NameTooShort);
        }
        if self.name.len() > 23 {
            return Err(CharacterCreationError::NameTooLong);
        }
        if !self.name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(CharacterCreationError::NameInvalidCharacters);
        }

        // Check forbidden names
        let forbidden_words = ["gm", "admin", "test", "bot"];
        let name_lower = self.name.to_lowercase();
        for word in &forbidden_words {
            if name_lower.contains(word) {
                return Err(CharacterCreationError::NameForbidden);
            }
        }

        // Stats validation (total should be 30 for starting character)
        let total_stats = self.str + self.agi + self.vit + self.int + self.dex + self.luk;
        if total_stats != 30 {
            return Err(CharacterCreationError::InvalidStats);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum CharacterCreationError {
    #[error("Character name cannot be empty")]
    NameEmpty,
    #[error("Character name must be at least 4 characters")]
    NameTooShort,
    #[error("Character name cannot exceed 23 characters")]
    NameTooLong,
    #[error("Character name can only contain letters, numbers, and underscores")]
    NameInvalidCharacters,
    #[error("Character name contains forbidden words")]
    NameForbidden,
    #[error("Invalid stat distribution")]
    InvalidStats,
    #[error("Server error: {0}")]
    ServerError(String),
}

// Convert from network protocol CharacterInfo to domain Character
impl From<crate::infrastructure::networking::protocols::ro_char::CharacterInfo> for CharacterData {
    fn from(info: crate::infrastructure::networking::protocols::ro_char::CharacterInfo) -> Self {
        Self {
            char_id: info.char_id,
            name: info.name,
            class: JobClass::from(info.class),
            base_level: info.base_level,
            job_level: info.job_level as u16,
            base_exp: info.base_exp,
            job_exp: info.job_exp,
            hp: info.hp,
            max_hp: info.max_hp,
            sp: info.sp,
            max_sp: info.max_sp,
            zeny: info.zeny,
            str: info.str,
            agi: info.agi,
            vit: info.vit,
            int: info.int,
            dex: info.dex,
            luk: info.luk,
            hair_style: info.hair,
            hair_color: info.hair_color,
            clothes_color: info.clothes_color,
            weapon: info.weapon,
            shield: info.shield,
            head_top: info.head_top,
            head_mid: info.head_mid,
            head_bottom: info.head_bottom,
            robe: info.robe,
            last_map: info.last_map,
            delete_date: if info.delete_date > 0 {
                Some(info.delete_date)
            } else {
                None
            },
            sex: Gender::from(info.sex),
        }
    }
}
