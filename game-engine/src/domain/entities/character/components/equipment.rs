use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
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


impl EquipmentSet {
    pub fn get_item(&self, slot: EquipmentSlot) -> &Option<EquipmentItem> {
        match slot {
            EquipmentSlot::HeadTop => &self.head_top,
            EquipmentSlot::HeadMid => &self.head_mid,
            EquipmentSlot::HeadBottom => &self.head_bottom,
            EquipmentSlot::Weapon => &self.weapon,
            EquipmentSlot::Shield => &self.shield,
            EquipmentSlot::Armor => &self.armor,
            EquipmentSlot::Garment => &self.garment,
            EquipmentSlot::Shoes => &self.shoes,
            EquipmentSlot::Accessory1 => &self.accessories[0],
            EquipmentSlot::Accessory2 => &self.accessories[1],
        }
    }

    pub fn set_item(&mut self, slot: EquipmentSlot, item: Option<EquipmentItem>) {
        match slot {
            EquipmentSlot::HeadTop => self.head_top = item,
            EquipmentSlot::HeadMid => self.head_mid = item,
            EquipmentSlot::HeadBottom => self.head_bottom = item,
            EquipmentSlot::Weapon => self.weapon = item,
            EquipmentSlot::Shield => self.shield = item,
            EquipmentSlot::Armor => self.armor = item,
            EquipmentSlot::Garment => self.garment = item,
            EquipmentSlot::Shoes => self.shoes = item,
            EquipmentSlot::Accessory1 => self.accessories[0] = item,
            EquipmentSlot::Accessory2 => self.accessories[1] = item,
        }
    }

    pub fn iter_equipped(&self) -> impl Iterator<Item = (EquipmentSlot, &EquipmentItem)> {
        [
            (EquipmentSlot::HeadTop, &self.head_top),
            (EquipmentSlot::HeadMid, &self.head_mid),
            (EquipmentSlot::HeadBottom, &self.head_bottom),
            (EquipmentSlot::Weapon, &self.weapon),
            (EquipmentSlot::Shield, &self.shield),
            (EquipmentSlot::Armor, &self.armor),
            (EquipmentSlot::Garment, &self.garment),
            (EquipmentSlot::Shoes, &self.shoes),
            (EquipmentSlot::Accessory1, &self.accessories[0]),
            (EquipmentSlot::Accessory2, &self.accessories[1]),
        ]
        .into_iter()
        .filter_map(|(slot, item)| item.as_ref().map(|item| (slot, item)))
    }
}

impl EquipmentSlot {
    pub fn z_order(&self) -> f32 {
        match self {
            EquipmentSlot::Garment => 10.0,
            EquipmentSlot::Armor => 20.0,
            EquipmentSlot::Shield => 25.0,
            EquipmentSlot::Weapon => 30.0,
            EquipmentSlot::Shoes => 5.0,
            EquipmentSlot::HeadBottom => 35.0,
            EquipmentSlot::HeadMid => 40.0,
            EquipmentSlot::HeadTop => 45.0,
            EquipmentSlot::Accessory1 => 50.0,
            EquipmentSlot::Accessory2 => 51.0,
        }
    }
}
