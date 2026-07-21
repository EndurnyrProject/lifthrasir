pub mod action_sync;
pub mod body_sync;
pub mod cart;
pub mod events;
pub mod head_sync;
pub mod headgear_sync;
pub mod job_change;
pub mod spawn;
pub mod update;
pub mod weapon_motion;
pub mod weapon_sync;

pub use action_sync::{
    sync_mob_sprite_action, sync_mob_sprite_direction, sync_player_sprite_action,
    sync_player_sprite_direction,
};
pub use body_sync::{sync_mob_body_layer, sync_player_body_layer};
pub use cart::{apply_cart_mount, finalize_cart_layer, sync_cart_layer};
pub use events::{
    EquipmentChangeEvent, StatusEffectVisualEvent, handle_equipment_changes,
    handle_status_effect_visuals,
};
pub use head_sync::sync_player_head_layer;
pub use headgear_sync::sync_headgear_layer;
pub use job_change::apply_base_look_changes;
pub use spawn::spawn_sprite_hierarchy;
pub use update::cleanup_orphaned_sprites;
pub use weapon_motion::sync_weapon_combat_motion;
pub use weapon_sync::sync_weapon_layer;

use bevy::prelude::*;

/// Point the layer material at `texture`.
///
/// NOTE: the write is deliberately unconditional. Marking the material
/// modified every frame is load-bearing: Bevy's retained transparent phase
/// freezes an item's sort position (`mesh_center`) at queue time and only
/// re-queues on respecialization (e.g. a material change). The unit's
/// near-coplanar layer quads rely on that per-frame re-queue for a fresh,
/// correctly ordered blend sort as units and the camera move; gating this
/// write scrambles the body/head/headgear stacking. If it ever needs to be
/// cheaper, re-queueing must be forced another way.
pub(crate) fn set_layer_texture(
    materials: &mut Assets<StandardMaterial>,
    handle: &Handle<StandardMaterial>,
    texture: &Handle<Image>,
) {
    if let Some(mut material) = materials.get_mut(handle) {
        material.base_color_texture = Some(texture.clone());
    }
}
