use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use super::types::ObjectType;

#[derive(Resource)]
#[auto_init_resource(plugin = crate::app::entity_hover_plugin::EntityHoverDomainPlugin)]
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

#[derive(EntityEvent, Debug, Clone)]
pub struct EntityHoverEntered {
    #[event_target]
    pub entity: Entity,
    pub entity_id: u32,
    pub object_type: ObjectType,
}

#[derive(EntityEvent, Debug, Clone)]
pub struct EntityHoverExited {
    #[event_target]
    pub entity: Entity,
}
