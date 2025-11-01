use super::events::{DespawnEntity, RequestEntityVanish, SpawnEntity};
use bevy::prelude::*;

pub struct EntitySpawningPlugin;

impl Plugin for EntitySpawningPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SpawnEntity>()
            .add_message::<DespawnEntity>()
            .add_message::<RequestEntityVanish>()
            .add_systems(
                Update,
                (
                    // Handle vanish requests and mark entities for pending despawn
                    super::systems::handle_vanish_request_system,
                    // Check pending despawns and emit DespawnEntity when ready
                    super::systems::check_pending_despawns_system,
                    // Spawn new entities
                    super::systems::spawn_network_entity_system,
                    // Despawn entities (handles DespawnEntity events)
                    super::systems::despawn_network_entity_system,
                    // Cleanup registry
                    super::systems::cleanup_despawned_entities_system,
                )
                    .chain()
                    .before(
                        crate::domain::entities::sprite_rendering::systems::update_sprite_transforms,
                    ),
            );
    }
}
