use bevy_auto_plugin::prelude::*;

/// Entity Spawning Plugin
///
/// This plugin handles entity spawning and despawning via observers.
/// Network message registration is handled by the EntitySpawningPlugin wrapper
/// in domain/entities/spawning/plugin.rs.
///
/// Registered observers:
/// - on_entity_vanish_request
/// - on_despawn_entity
///
/// Registered systems (chained before update_sprite_transforms):
/// - bridge_vanish_requests_system
/// - check_pending_despawns_system
/// - spawn_network_entity_system
/// - cleanup_despawned_entities_system
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct EntitySpawningDomainPlugin;
