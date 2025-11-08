use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::{auto_add_event, auto_add_system};

use super::components::{CharacterAppearance, CharacterData};
use crate::domain::entities::sprite_rendering::{
    EntitySpriteData, EntitySpriteInfo, SpawnSpriteEvent,
};

#[derive(Message)]
#[auto_add_event(plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin)]
pub struct SpawnCharacterSpriteEvent {
    pub character_entity: Entity,
    pub spawn_position: Vec3,
}

/// System that converts character-specific spawn events to generic sprite spawn events
#[auto_add_system(
    plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin,
    schedule = Update
)]
pub fn forward_character_sprite_events(
    mut character_events: MessageReader<SpawnCharacterSpriteEvent>,
    mut sprite_events: MessageWriter<SpawnSpriteEvent>,
    characters: Query<(&CharacterData, &CharacterAppearance)>,
) {
    for event in character_events.read() {
        let Ok((data, appearance)) = characters.get(event.character_entity) else {
            warn!(
                "Character entity {:?} not found for sprite spawn",
                event.character_entity
            );
            continue;
        };

        sprite_events.write(SpawnSpriteEvent {
            entity: event.character_entity,
            position: event.spawn_position,
            sprite_info: EntitySpriteInfo {
                sprite_data: EntitySpriteData::Character {
                    job_class: data.job_id,
                    gender: appearance.gender,
                    head: appearance.hair_style,
                },
            },
        });
    }
}
