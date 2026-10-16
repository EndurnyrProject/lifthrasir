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
pub fn set_layer_texture(
    materials: &mut Assets<StandardMaterial>,
    handle: &Handle<StandardMaterial>,
    texture: &Handle<Image>,
) {
    if let Some(mut material) = materials.get_mut(handle) {
        material.base_color_texture = Some(texture.clone());
    }
}
