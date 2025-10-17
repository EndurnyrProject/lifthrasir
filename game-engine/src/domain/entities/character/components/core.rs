use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Gender enum - shared across the application
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

/// Job class enum - represents character professions
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

#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct CharacterData {
    pub name: String,
    pub job_id: u16,
    pub level: u32,
    pub experience: u64,
    pub stats: CharacterStats,
    pub slot: u8,
}

#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CharacterStats {
    pub str: u16,
    pub agi: u16,
    pub vit: u16,
    pub int: u16,
    pub dex: u16,
    pub luk: u16,
    pub max_hp: u32,
    pub current_hp: u32,
    pub max_sp: u32,
    pub current_sp: u32,
}

#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct CharacterAppearance {
    pub gender: Gender,
    pub hair_style: u16,
    pub hair_color: u16,
    pub clothes_color: u16,
}

#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct CharacterMeta {
    pub char_id: u32,
    pub last_map: String,
    pub delete_date: Option<u32>,
}

/// DTO for passing character data through events and across boundaries
/// This replaces the legacy CharacterData model from domain::character::models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterInfo {
    // CharacterData fields
    pub name: String,
    pub job_id: u16,
    pub level: u32,
    pub experience: u64,
    pub stats: CharacterStats,
    pub slot: u8,

    // CharacterAppearance fields
    pub gender: Gender,
    pub hair_style: u16,
    pub hair_color: u16,
    pub clothes_color: u16,

    // CharacterMeta fields
    pub char_id: u32,
    pub last_map: String,
    pub delete_date: Option<u32>,
}

impl CharacterInfo {
    /// Convert CharacterInfo DTO into the three ECS components
    pub fn into_components(self) -> (CharacterData, CharacterAppearance, CharacterMeta) {
        (
            CharacterData {
                name: self.name,
                job_id: self.job_id,
                level: self.level,
                experience: self.experience,
                stats: self.stats,
                slot: self.slot,
            },
            CharacterAppearance {
                gender: self.gender,
                hair_style: self.hair_style,
                hair_color: self.hair_color,
                clothes_color: self.clothes_color,
            },
            CharacterMeta {
                char_id: self.char_id,
                last_map: self.last_map,
                delete_date: self.delete_date,
            },
        )
    }
}

/// Conversion from network protocol CharacterInfo to domain CharacterInfo DTO
impl From<crate::infrastructure::networking::protocol::character::CharacterInfo> for CharacterInfo {
    fn from(net: crate::infrastructure::networking::protocol::character::CharacterInfo) -> Self {
        Self {
            name: net.name,
            job_id: net.class,
            level: net.base_level as u32,
            experience: net.base_exp,
            stats: CharacterStats {
                str: net.str as u16,
                agi: net.agi as u16,
                vit: net.vit as u16,
                int: net.int as u16,
                dex: net.dex as u16,
                luk: net.luk as u16,
                max_hp: net.max_hp as u32,
                current_hp: net.hp as u32,
                max_sp: net.max_sp as u32,
                current_sp: net.sp as u32,
            },
            slot: net.char_num,
            gender: Gender::from(net.sex),
            hair_style: net.hair,
            hair_color: net.hair_color,
            clothes_color: net.clothes_color,
            char_id: net.char_id,
            last_map: net.last_map,
            delete_date: if net.delete_date > 0 {
                Some(net.delete_date)
            } else {
                None
            },
        }
    }
}

impl Default for CharacterStats {
    fn default() -> Self {
        Self {
            str: 1,
            agi: 1,
            vit: 1,
            int: 1,
            dex: 1,
            luk: 1,
            max_hp: 100,
            current_hp: 100,
            max_sp: 100,
            current_sp: 100,
        }
    }
}

impl CharacterStats {
    pub fn total(&self) -> u16 {
        self.str + self.agi + self.vit + self.int + self.dex + self.luk
    }

    pub fn is_valid_starting_stats(&self) -> bool {
        self.total() == 30
            && self.str >= 1
            && self.agi >= 1
            && self.vit >= 1
            && self.int >= 1
            && self.dex >= 1
            && self.luk >= 1
    }
}
