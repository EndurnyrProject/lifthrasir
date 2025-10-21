use bevy::prelude::*;
use super::events::{SpawnEntity, DespawnEntity};

pub struct EntitySpawningPlugin;

impl Plugin for EntitySpawningPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_message::<SpawnEntity>()
            .add_message::<DespawnEntity>()
            .add_systems(
                Update,
                (
                    super::systems::spawn_network_entity_system,
                    super::systems::despawn_network_entity_system,
                    super::systems::cleanup_despawned_entities_system,
                ).chain()
            );
    }
}
