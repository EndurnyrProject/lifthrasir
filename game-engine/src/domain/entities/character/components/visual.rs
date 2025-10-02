use super::equipment::EquipmentSlot;
use crate::infrastructure::assets::loaders::{RoActAsset, RoSpriteAsset};
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Component, Debug)]
pub struct CharacterSprite {
    pub body_sprite: Entity,
    pub equipment_layers: HashMap<EquipmentSlot, Entity>,
    pub effect_layers: Vec<Entity>,
    pub current_action: u8,
    pub current_frame: u8,
    pub animation_timer: Timer,
}

#[derive(Component, Debug, Clone)]
pub struct RoSpriteLayer {
    pub sprite_handle: Handle<RoSpriteAsset>,
    pub action_handle: Handle<RoActAsset>,
    pub layer_type: SpriteLayerType,
    pub z_offset: f32,
}

#[derive(Debug, Clone)]
pub enum SpriteLayerType {
    Body,
    Equipment(EquipmentSlot),
    Effect(EffectType),
    Shadow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectType {
    Blessing,
    Curse,
    Poison,
    Freeze,
    Stone,
    Stun,
    Sleep,
    Concentration,
    Endure,
    // Add more status effects as needed
}

#[derive(Component, Debug)]
pub struct CharacterDirection {
    pub facing: Direction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    South = 0,
    SouthWest = 1,
    West = 2,
    NorthWest = 3,
    North = 4,
    NorthEast = 5,
    East = 6,
    SouthEast = 7,
}

impl Default for CharacterSprite {
    fn default() -> Self {
        Self {
            body_sprite: Entity::PLACEHOLDER,
            equipment_layers: HashMap::new(),
            effect_layers: Vec::new(),
            current_action: 0, // Idle action
            current_frame: 0,
            animation_timer: Timer::from_seconds(0.15, TimerMode::Repeating),
        }
    }
}

impl Default for CharacterDirection {
    fn default() -> Self {
        Self {
            facing: Direction::South,
        }
    }
}

impl Direction {
    pub fn to_sprite_direction(self) -> u8 {
        self as u8
    }

    pub fn from_angle(angle: f32) -> Self {
        let normalized = ((angle % (2.0 * std::f32::consts::PI) + 2.0 * std::f32::consts::PI)
            % (2.0 * std::f32::consts::PI))
            * 180.0
            / std::f32::consts::PI;

        match normalized as u32 {
            337..=360 | 0..=22 => Direction::North,
            23..=67 => Direction::NorthEast,
            68..=112 => Direction::East,
            113..=157 => Direction::SouthEast,
            158..=202 => Direction::South,
            203..=247 => Direction::SouthWest,
            248..=292 => Direction::West,
            293..=336 => Direction::NorthWest,
            _ => Direction::South,
        }
    }
}

impl CharacterSprite {
    pub fn has_effect_visual(&self, effect_type: EffectType) -> bool {
        // For now, we'll implement this simply
        // In a full implementation, we'd check if any effect layer
        // corresponds to the given effect type
        false
    }

    pub fn play_action(&mut self, action: ActionType) {
        self.current_action = action as u8;
        self.current_frame = 0;
        self.animation_timer.reset();
    }

    pub fn set_direction(&mut self, direction: Direction) {
        // This would update the sprite direction
        // For now, just store it - actual implementation would
        // update the sprite rendering based on direction
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    Idle = 0,
    Walk = 1,
    Sit = 2,
    Attack = 3,
    Hit = 4,
    Dead = 5,
    Cast = 6,
    Special = 7,
}

impl From<EffectType> for SpriteLayerType {
    fn from(effect_type: EffectType) -> Self {
        SpriteLayerType::Effect(effect_type)
    }
}

impl SpriteLayerType {
    pub fn from_name(name: &str) -> Self {
        match name {
            "Body" => SpriteLayerType::Body,
            "Equipment/HeadBottom" => SpriteLayerType::Equipment(EquipmentSlot::HeadBottom),
            "Equipment/HeadMid" => SpriteLayerType::Equipment(EquipmentSlot::HeadMid),
            "Equipment/HeadTop" => SpriteLayerType::Equipment(EquipmentSlot::HeadTop),
            _ => SpriteLayerType::Body, // Default fallback
        }
    }
}

impl Default for RoSpriteLayer {
    fn default() -> Self {
        Self {
            sprite_handle: Handle::default(),
            action_handle: Handle::default(),
            layer_type: SpriteLayerType::Body,
            z_offset: 0.0,
        }
    }
}
