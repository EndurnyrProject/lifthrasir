use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Debug, Clone, Serialize, Deserialize, Default)]
pub struct EquipmentSet {
    pub head_top: Option<EquipmentItem>,
    pub head_mid: Option<EquipmentItem>,
    pub head_bottom: Option<EquipmentItem>,
    pub weapon: Option<EquipmentItem>,
    pub shield: Option<EquipmentItem>,
    pub armor: Option<EquipmentItem>,
    pub garment: Option<EquipmentItem>,
    pub shoes: Option<EquipmentItem>,
    pub accessories: [Option<EquipmentItem>; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquipmentItem {
    pub item_id: u32,
    pub sprite_id: u16,
    pub refinement: u8,
    pub enchantments: Vec<u16>,
    pub options: Vec<EquipmentOption>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EquipmentSlot {
    HeadTop,
    HeadMid,
    HeadBottom,
    Weapon,
    Shield,
    Armor,
    Garment,
    Shoes,
    Accessory1,
    Accessory2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquipmentOption {
    pub option_type: u16,
    pub option_value: u16,
    pub option_param: u8,
}
