use super::components::EntitySpriteInfo;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

#[derive(Message)]
#[auto_add_message(plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin)]
pub struct SpawnSpriteEvent {
    pub entity: Entity,
    pub position: Vec3,
    pub sprite_info: EntitySpriteInfo,
}

/// Observer event to request sprite spawning for an entity
///
/// This event is triggered via `commands.entity().trigger()` after entity creation.
/// Using an observer ensures the entity exists in the ECS world before sprite spawn
/// is attempted, avoiding race conditions with buffered commands.
#[derive(EntityEvent, Debug, Clone)]
pub struct RequestSpriteSpawn {
    #[event_target]
    pub entity: Entity,
    pub position: Vec3,
    pub sprite_info: EntitySpriteInfo,
}
