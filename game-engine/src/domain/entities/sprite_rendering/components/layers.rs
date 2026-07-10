use bevy::prelude::*;

/// Marker component for head sprite layers.
#[derive(Component, Default)]
pub struct HeadLayer;

/// Marker component for the pushcart sprite layer.
///
/// The cart is a non-equipment attachment layer (`LAYER_CART`, drawn behind the
/// body) spawned from the unit's `effect_state` cart bit. The marker keys the
/// cart's per-frame sync and its despawn on unmount, and its presence on a child
/// is the parent's mount state (no separate mounted flag needed).
#[derive(Component, Default)]
pub struct CartLayer;

/// Body publishes its attach point, frame index, and layer position each frame for head to read.
/// Head uses the same frame index to get its attach point for synchronized positioning.
#[derive(Component, Default)]
pub struct BodyAttachPoint {
    pub attach_point: Vec2,
    pub frame_index: usize,
    pub layer_pos: Vec2,
}

/// Head publishes its attach point, frame index, and layer position each frame for headgear to read.
/// Headgear uses the head's frame index and attach data to align to the head, exactly the way the
/// head aligns to the body via `BodyAttachPoint`.
#[derive(Component, Default)]
pub struct HeadAttachPoint {
    pub attach_point: Vec2,
    pub frame_index: usize,
    pub layer_pos: Vec2,
}

/// Head stores reference to body entity for attach point lookup.
/// Used to calculate head positioning relative to body.
#[derive(Component)]
pub struct HeadAttachment {
    pub body_entity: Entity,
}
