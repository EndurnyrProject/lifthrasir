use super::events::{RequestEntityVanish, SpawnEntity};
use crate::app::entity_spawning_plugin::EntitySpawningDomainPlugin;
use bevy::prelude::*;

/// Entity Spawning Plugin (Wrapper)
///
/// Composes entity spawning functionality with proper dependency order:
/// 1. Network protocol messages (infrastructure-level)
/// 2. EntitySpawningDomainPlugin (auto-plugin with observers and systems)
///
/// # System Flow
///
/// 1. `bridge_vanish_requests_system` - Converts network messages to observer events
/// 2. `check_pending_despawns_system` - Checks pending despawns and triggers DespawnEntity
/// 3. `spawn_network_entity_system` - Spawns new entities
/// 4. `cleanup_despawned_entities_system` - Cleans up registry
///
/// All systems run `.before(update_sprite_transforms)` to prevent race conditions.
pub struct EntitySpawningPlugin;

impl Plugin for EntitySpawningPlugin {
    fn build(&self, app: &mut App) {
        // Register network protocol messages (infrastructure-level)
        app.add_message::<SpawnEntity>()
            .add_message::<RequestEntityVanish>();

        // Add entity spawning domain plugin (auto-plugin with observers and systems)
        app.add_plugins(EntitySpawningDomainPlugin);

        info!("EntitySpawningPlugin initialized");
    }
}
