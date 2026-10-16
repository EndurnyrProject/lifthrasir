use crate::rendering::Direction;
use bevy::prelude::*;
use lifthrasir_assets::animation::RoAnimationAsset;
use moonshine_tag::Tag;
use serde::{Deserialize, Serialize};

/// Maps a generated pickable mesh to the caller-owned actor root.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorVisualRoot(pub Entity);

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

/// Marker component for child entities that render a sprite layer.
/// Child entities have Mesh3d + MeshMaterial3d + Billboard components.
/// The parent entity holds PlayerSprite/MobSprite for animation state.
#[derive(Component, Clone, Debug)]
pub struct RenderLayer {
    /// The layer tag for z-ordering (LAYER_BODY, LAYER_HEAD, etc.)
    pub layer: Tag,

    /// Handle to the animation asset this layer uses
    pub animation: Handle<RoAnimationAsset>,

    /// Equipment slot this layer represents (None for body/head)
    pub equipment_slot: Option<EquipmentSlot>,

    /// Texture handles to keep images alive (prevents GC)
    pub textures: Vec<Handle<Image>>,
}

impl RenderLayer {
    /// Create a new render layer for body
    pub fn body(
        animation: Handle<RoAnimationAsset>,
        layer: Tag,
        textures: Vec<Handle<Image>>,
    ) -> Self {
        Self {
            layer,
            animation,
            equipment_slot: None,
            textures,
        }
    }

    /// Create a new render layer for equipment
    pub fn equipment(
        animation: Handle<RoAnimationAsset>,
        layer: Tag,
        slot: EquipmentSlot,
        textures: Vec<Handle<Image>>,
    ) -> Self {
        Self {
            layer,
            animation,
            equipment_slot: Some(slot),
            textures,
        }
    }
}

#[derive(Component, Debug)]
pub struct CharacterDirection {
    pub facing: Direction,
}

impl Default for CharacterDirection {
    fn default() -> Self {
        Self {
            facing: Direction::South,
        }
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
    ReadyFight = 8,
    Attack1 = 9,
    Attack2 = 10,
}

/// Records the resolved attack motion of a character's equipped weapon so an
/// armed attack plays the weapon's real swing (ATTACK1/2/3 vary by weapon).
#[derive(Component, Clone, Copy)]
pub struct CombatMotion {
    pub attack: ActionType,
}
