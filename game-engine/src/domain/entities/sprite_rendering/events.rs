use super::components::EntitySpriteInfo;
use bevy::prelude::*;

#[derive(Message)]
pub struct SpawnSpriteEvent {
    pub entity: Entity,
    pub position: Vec3,
    pub sprite_info: EntitySpriteInfo,
}
