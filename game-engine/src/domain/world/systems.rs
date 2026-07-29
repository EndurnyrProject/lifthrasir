use crate::core::GameState;
use crate::domain::system_sets::WorldLoadingSystems;
use crate::domain::world::map::MapData;
use crate::domain::world::map_loader::MapRequestLoader;
use crate::domain::world::map_scoped::MapScoped;
use crate::domain::world::spawn_context::MapSpawnContext;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

#[auto_add_system(
    plugin = crate::domain::world::WorldDomainPlugin,
    schedule = Update,
    config(in_set = WorldLoadingSystems::LoaderSetup)
)]
pub fn setup_unified_map_loading(
    mut commands: Commands,
    spawn_context: Option<Res<MapSpawnContext>>,
    existing_loaders: Query<&MapRequestLoader>,
) {
    // Only run if MapSpawnContext exists and no loader already exists
    let Some(context) = spawn_context else {
        return;
    };

    // Check if a map loader already exists to prevent duplicate loading
    if !existing_loaders.is_empty() {
        return;
    }

    // FAIL-FAST: Panic if map name is invalid/empty
    assert!(
        !context.map_name.is_empty(),
        "MapSpawnContext has invalid empty map name!"
    );

    info!(
        "Loading map: {} at spawn ({}, {})",
        context.map_name, context.spawn_x, context.spawn_y
    );

    commands.spawn((MapRequestLoader::new(context.map_name.clone()), MapScoped));
    debug!(
        "Spawned MapRequestLoader entity for map '{}'",
        context.map_name
    );
}

/// Cleanup system to despawn stale MapRequestLoader entities
/// Runs when exiting Loading or Connecting states to prevent stale entities from blocking future loads
#[auto_add_system(
    plugin = crate::domain::world::WorldDomainPlugin,
    schedule = OnExit(GameState::Loading)
)]
#[auto_add_system(
    plugin = crate::domain::world::WorldDomainPlugin,
    schedule = OnExit(GameState::Connecting)
)]
pub fn cleanup_map_loading_state(
    mut commands: Commands,
    query: Query<Entity, (With<MapRequestLoader>, Without<MapData>)>,
) {
    let count = query.iter().count();
    if count > 0 {
        debug!(
            "cleanup_map_loading_state: Despawning {} stale MapRequestLoader entities (excluding successful loads with MapData)",
            count
        );
        for entity in query.iter() {
            commands.entity(entity).despawn();
        }
    }
}

/// State verification system - logs when Loading state is entered
/// This helps diagnose if state transitions are working correctly
#[auto_add_system(
    plugin = crate::domain::world::WorldDomainPlugin,
    schedule = OnEnter(GameState::Loading)
)]
pub fn on_enter_loading_state(spawn_context: Option<Res<MapSpawnContext>>) {
    if let Some(context) = spawn_context {
        debug!(
            "🎯 ENTERED GameState::Loading - MapSpawnContext found for map '{}'",
            context.map_name
        );
    } else {
        warn!(
            "⚠️ ENTERED GameState::Loading - BUT MapSpawnContext NOT FOUND! This will cause setup_unified_map_loading to fail"
        );
    }
}

/// Monitors current GameState and logs when it changes
/// This helps diagnose if state transitions are actually being applied
#[auto_add_system(
    plugin = crate::domain::world::WorldDomainPlugin,
    schedule = Update
)]
pub fn monitor_game_state(current_state: Res<State<GameState>>) {
    if current_state.is_changed() {
        debug!("🔄 GameState CHANGED to: {:?}", current_state.get());
    }
}
