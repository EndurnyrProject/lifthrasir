use crate::domain::entities::character::components::{equipment::EquipmentSlot, Gender};
use crate::infrastructure::assets::loaders::{RoActAsset, RoSpriteAsset};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

#[derive(Component, Debug)]
pub struct SpriteObjectTree {
    pub root: Entity,
}

impl SpriteObjectTree {
    pub fn get_root_entity(&self) -> Entity {
        self.root
    }
}

#[derive(Component)]
pub struct SpriteHierarchy {
    pub parent_entity: Entity,
    pub layer_type: SpriteLayerType,
}

#[derive(Debug, Clone)]
pub enum SpriteLayerType {
    Body,
    Head,
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
}

#[derive(Clone, Debug)]
pub enum EntitySpriteData {
    Character {
        job_id: u16,
        gender: Gender,
        head: u16,
    },
    Mob {
        sprite_name: String,
    },
    Npc {
        sprite_name: String,
    },
}

#[derive(Component, Clone, Debug)]
pub struct EntitySpriteInfo {
    pub sprite_data: EntitySpriteData,
}

#[derive(Component, Debug, Clone)]
pub struct RoSpriteLayer {
    pub sprite_handle: Handle<RoSpriteAsset>,
    pub action_handle: Handle<RoActAsset>,
    pub layer_type: SpriteLayerType,
    pub z_offset: f32,
}

#[derive(Resource)]
#[auto_init_resource(plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin)]
pub struct SpriteHierarchyConfig {
    pub default_z_spacing: f32,
    pub effect_z_offset: f32,
    pub shadow_z_offset: f32,
}

impl Default for SpriteHierarchyConfig {
    fn default() -> Self {
        Self {
            default_z_spacing: 0.01,
            effect_z_offset: 0.1,
            shadow_z_offset: -0.05,
        }
    }
}
