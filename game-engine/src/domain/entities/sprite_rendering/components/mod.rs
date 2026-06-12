mod layers;
mod ro_sprite;

pub use layers::{BodyAttachPoint, HeadAttachment, HeadLayer};
pub use ro_sprite::{MobSprite, PlayerSprite, RoSpriteGeneric};

use std::collections::HashMap;

use crate::domain::entities::character::components::{equipment::EquipmentSlot, Gender};
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use moonshine_tag::Tag;

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
    /// Iterate over all animation handles for rendering
    pub fn iter_layers(
        &self,
    ) -> impl Iterator<Item = (&Handle<RoAnimationAsset>, Option<EquipmentSlot>)> {
        std::iter::once((&self.body, None))
            .chain(std::iter::once((&self.head, None)))
            .chain(
                self.equipment
                    .iter()
                    .map(|(slot, handle)| (handle, Some(*slot))),
            )
    }

    /// Set equipment for a slot
    pub fn set_equipment(&mut self, slot: EquipmentSlot, animation: Handle<RoAnimationAsset>) {
        self.equipment.insert(slot, animation);
    }

    /// Remove equipment from a slot
    pub fn remove_equipment(&mut self, slot: EquipmentSlot) -> Option<Handle<RoAnimationAsset>> {
        self.equipment.remove(&slot)
    }

    /// Check if equipment is present in a slot
    pub fn has_equipment(&self, slot: EquipmentSlot) -> bool {
        self.equipment.contains_key(&slot)
    }
}

/// Marker component for child entities that render a sprite layer.
/// Child entities have Mesh3d + MeshMaterial3d + Billboard components.
/// The parent entity holds RoSprite for animation state.
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
    },
    Mob {
        sprite_name: String,
    },
    Npc {
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
