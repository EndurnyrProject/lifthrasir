use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

#[derive(Component, Reflect)]
pub struct HoveredEntity;

#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::app::entity_hover_plugin::EntityHoverDomainPlugin)]
pub struct CurrentlyHoveredEntity {
    pub entity: Option<Entity>,
}

#[derive(EntityEvent, Debug, Clone)]
pub struct EntityHoverEntered {
    #[event_target]
    pub entity: Entity,
    pub entity_id: u32,
}

#[derive(EntityEvent, Debug, Clone)]
pub struct EntityHoverExited {
    #[event_target]
    pub entity: Entity,
}
