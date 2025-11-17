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
