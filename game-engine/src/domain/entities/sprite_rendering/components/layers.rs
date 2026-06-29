use bevy::prelude::*;

/// Marker component for head sprite layers.
#[derive(Component, Default)]
pub struct HeadLayer;

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
