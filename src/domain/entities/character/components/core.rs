use crate::domain::character::models::Gender;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

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

// Conversion from existing CharacterData model
impl From<crate::domain::character::models::CharacterData> for CharacterData {
    fn from(old: crate::domain::character::models::CharacterData) -> Self {
        Self {
            name: old.name,
            job_id: old.class as u16,
            level: old.base_level as u32,
            experience: old.base_exp,
            stats: CharacterStats {
                str: old.str as u16,
                agi: old.agi as u16,
                vit: old.vit as u16,
                int: old.int as u16,
                dex: old.dex as u16,
                luk: old.luk as u16,
                max_hp: old.max_hp as u32,
                current_hp: old.hp as u32,
                max_sp: old.max_sp as u32,
                current_sp: old.sp as u32,
            },
            slot: 0, // Will be set by the spawning system
        }
    }
}

impl From<crate::domain::character::models::CharacterData> for CharacterAppearance {
    fn from(old: crate::domain::character::models::CharacterData) -> Self {
        Self {
            gender: old.sex,
            hair_style: old.hair_style,
            hair_color: old.hair_color,
            clothes_color: old.clothes_color,
        }
    }
}

impl From<crate::domain::character::models::CharacterData> for CharacterMeta {
    fn from(old: crate::domain::character::models::CharacterData) -> Self {
        Self {
            char_id: old.char_id,
            last_map: old.last_map,
            delete_date: old.delete_date,
        }
    }
}
