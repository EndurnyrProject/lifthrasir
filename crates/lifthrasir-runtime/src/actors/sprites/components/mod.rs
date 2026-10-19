mod layers;
mod ro_sprite;

pub use layers::{
    BodyAttachPoint, CartLayer, FalconLayer, HeadAttachPoint, HeadAttachment, HeadLayer,
};
pub use ro_sprite::{MobSprite, PlayerSprite, RoSpriteGeneric};

use std::collections::HashMap;

use crate::actors::EquipmentSlot;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use lifthrasir_assets::Gender;
use lifthrasir_assets::animation::asset::RoAnimationAsset;

// =============================================================================
// PHASE 3: NEW COMPONENT STRUCTURE
// =============================================================================
// PlayerAppearance - holds animation handles for body, head, equipment
// RenderLayer - marker for child entities that render sprite layers
// ShadowRenderLayer - marker for shadow layer (special handling)
//
// Legacy components kept for compatibility:
// - EffectType - status effect visuals
// - SpriteHierarchyConfig - z-ordering configuration
// - EntitySpriteData/EntitySpriteInfo - spawn event data
// =============================================================================

/// Component for player entities that have compositable appearance.
/// Holds handles to animation assets for body, head, and equipment.
/// Used with Billboard child entities for actual rendering.
#[derive(Component, Clone, Debug, Default)]
pub struct PlayerAppearance {
    /// Body animation asset (job sprite)
    pub body: Handle<RoAnimationAsset>,

    /// Head animation asset (hair style)
    pub head: Handle<RoAnimationAsset>,

    /// Equipment animation assets by slot
    pub equipment: HashMap<EquipmentSlot, Handle<RoAnimationAsset>>,

    /// Shadow texture (single image, not animated)
    pub shadow: Handle<Image>,
}

impl PlayerAppearance {
    /// Set equipment for a slot
    pub fn set_equipment(&mut self, slot: EquipmentSlot, animation: Handle<RoAnimationAsset>) {
        self.equipment.insert(slot, animation);
    }

    /// Remove equipment from a slot
    pub fn remove_equipment(&mut self, slot: EquipmentSlot) -> Option<Handle<RoAnimationAsset>> {
        self.equipment.remove(&slot)
    }
}

pub use crate::actors::RenderLayer;

/// Marker component for shadow layer child entity.
/// Shadows are flat quads with different rotation behavior.
#[derive(Component, Clone, Debug, Default)]
pub struct ShadowRenderLayer;

/// Marker for entities waiting for their animation assets to load.
/// Removed after child render layers are spawned.
#[derive(Component, Clone, Debug, Default)]
pub struct PendingRenderLayers;

/// Describes what kind of sprite to load for an entity
#[derive(Clone, Debug)]
pub enum EntitySpriteData {
    Character {
        job_id: u16,
        gender: Gender,
        head: u16,
        hair_color: u16,
        /// Riding bit already set on entry, so the spawn requests the mounted
        /// body directly instead of racing a post-spawn swap.
        riding: bool,
    },
    Mob {
        sprite_name: String,
    },
    Npc {
        sprite_name: String,
    },
    Item {
        sprite_name: String,
    },
}

/// Component carrying sprite metadata for spawning
#[derive(Component, Clone, Debug)]
pub struct EntitySpriteInfo {
    pub sprite_data: EntitySpriteData,
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

#[derive(Resource)]
#[auto_init_resource(plugin = crate::actors::ActorRenderPlugin)]
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

/// The unit's body currently renders its mounted (Peco) sprite. Presence of the
/// marker *is* the rendered mount state, so repeated `effect_state` broadcasts
/// with the bit unchanged don't rebuild the body.
#[derive(Component)]
pub struct RidingPeco;
