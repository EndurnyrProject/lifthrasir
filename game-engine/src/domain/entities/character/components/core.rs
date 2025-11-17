use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Gender enum - shared across the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Gender {
    Female = 0,
    Male = 1,
}

impl Serialize for Gender {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for Gender {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        Ok(Gender::from(value))
    }
}

impl From<u8> for Gender {
    fn from(value: u8) -> Self {
        match value {
            0 => Gender::Female,
            _ => Gender::Male,
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
    pub sex: Gender,
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
                gender: self.sex,
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
            sex: Gender::from(net.sex),
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

/// Marker component for entities that should automatically follow terrain height.
/// Entities with this component will have their Y position updated every frame
/// to match the terrain altitude at their current X/Z position.
///
/// Used by the altitude system to enable automatic terrain following for grounded entities.
/// Flying units or entities that should not follow terrain should not have this component.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Grounded;
