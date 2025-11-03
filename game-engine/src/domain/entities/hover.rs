use bevy::prelude::*;

use super::types::ObjectType;

#[derive(Resource)]
pub struct HoverConfig {
    pub radius: f32,
}

impl Default for HoverConfig {
    fn default() -> Self {
        Self { radius: 30.0 }
    }
}

#[derive(Component, Reflect)]
pub struct HoveredEntity;

#[derive(Component, Reflect)]
pub struct Hoverable {
    pub screen_bounds: Rect,
}

impl Hoverable {
    pub fn new(screen_bounds: Rect) -> Self {
        Self { screen_bounds }
    }

    pub fn contains_point(&self, point: Vec2) -> bool {
        self.screen_bounds.contains(point)
    }
}

#[derive(Message)]
pub struct EntityHoverEntered {
    pub entity: Entity,
    pub entity_id: u32,
    pub object_type: ObjectType,
}

#[derive(Message)]
pub struct EntityHoverExited {
    pub entity: Entity,
}
