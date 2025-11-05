use super::events::{RequestEntityVanish, SpawnEntity};
use bevy::prelude::*;

pub struct EntitySpawningPlugin;

impl Plugin for EntitySpawningPlugin {
    fn build(&self, app: &mut App) {
        // Register network protocol messages
        app.add_message::<SpawnEntity>()
            .add_message::<RequestEntityVanish>();

        // Register observers for entity-targeted despawn events
        app.add_observer(super::systems::on_entity_vanish_request)
            .add_observer(super::systems::on_despawn_entity);

        app.add_systems(
            Update,
            (
                // Bridge: Convert network messages to observer events
                super::systems::bridge_vanish_requests_system,
                // Check pending despawns and trigger DespawnEntity observer when ready
                super::systems::check_pending_despawns_system,
                // Spawn new entities
                super::systems::spawn_network_entity_system,
                // Cleanup registry
                super::systems::cleanup_despawned_entities_system,
            )
                .chain()
                .before(
                    crate::domain::entities::sprite_rendering::systems::update_sprite_transforms,
                ),
        );

        info!("EntitySpawningPlugin initialized with observers");
    }
}
