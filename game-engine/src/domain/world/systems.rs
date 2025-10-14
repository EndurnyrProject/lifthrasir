use crate::domain::world::components::MapLoader;
use crate::domain::world::map::MapData;
use crate::domain::world::map_loader::MapRequestLoader;
use crate::domain::world::spawn_context::MapSpawnContext;
use crate::infrastructure::assets::loaders::{RoAltitudeAsset, RoGroundAsset, RoWorldAsset};
use bevy::prelude::*;

pub fn extract_map_from_unified_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut query: Query<(Entity, &mut MapRequestLoader), Without<MapLoader>>,
) {
    for (entity, mut map_loader) in query.iter_mut() {
        if map_loader.loaded {
            continue;
        }

        let map_name = &map_loader.map_name;
        debug!(
            "extract_map_from_unified_assets: Processing MapRequestLoader for map '{}'",
            map_name
        );

        // Strip .gat extension if present (server sends "new_1-1.gat")
        let base_name = map_name.trim_end_matches(".gat");

        // Use unified asset source with ro:// prefix
        let gnd_path = format!("ro://data/{}.gnd", base_name);
        let gat_path = format!("ro://data/{}.gat", base_name);
        let rsw_path = format!("ro://data/{}.rsw", base_name);

        debug!(
            "extract_map_from_unified_assets: Loading assets - GND: {}, GAT: {}, RSW: {}",
            gnd_path, gat_path, rsw_path
        );

        // Load assets through unified AssetServer - these return handles
        let ground_handle: Handle<RoGroundAsset> = asset_server.load(gnd_path);
        let altitude_handle: Handle<RoAltitudeAsset> = asset_server.load(gat_path);
        let world_handle: Handle<RoWorldAsset> = asset_server.load(rsw_path);

        info!(
            "Loaded map assets for '{}' through unified asset source",
            map_name
        );

        // Create MapLoader with the asset handles from AssetServer
        commands.entity(entity).insert(MapLoader {
            ground: ground_handle,
            altitude: Some(altitude_handle),
            world: Some(world_handle),
        });

        debug!("MapLoader component inserted for map '{}'", map_name);

        // Mark as loaded - AssetServer handles the actual loading asynchronously
        map_loader.loaded = true;
    }
}

pub fn setup_unified_map_loading(
    mut commands: Commands,
    spawn_context: Option<Res<MapSpawnContext>>,
    existing_loaders: Query<&MapRequestLoader>,
) {
    // Only run if MapSpawnContext exists and no loader already exists
    let Some(context) = spawn_context else {
        warn!("setup_unified_map_loading: MapSpawnContext not found - waiting for zone auth to complete");
        return; // Context not ready yet
    };

    // Check if a map loader already exists to prevent duplicate loading
    if !existing_loaders.is_empty() {
        return; // Already loading a map
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

    // Request loading of the specified map through the unified asset system
    commands.spawn(MapRequestLoader::new(context.map_name.clone()));
    info!(
        "Spawned MapRequestLoader entity for map '{}'",
        context.map_name
    );
}

/// Cleanup system to despawn stale MapRequestLoader entities
/// Runs when exiting Loading or Connecting states to prevent stale entities from blocking future loads
pub fn cleanup_map_loading_state(
    mut commands: Commands,
    query: Query<Entity, (With<MapRequestLoader>, Without<MapData>)>,
) {
    let count = query.iter().count();
    if count > 0 {
        info!(
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
pub fn on_enter_loading_state(spawn_context: Option<Res<MapSpawnContext>>) {
    if let Some(context) = spawn_context {
        info!(
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
pub fn monitor_game_state(current_state: Res<State<crate::core::GameState>>) {
    if current_state.is_changed() {
        info!("🔄 GameState CHANGED to: {:?}", current_state.get());
    }
}

/// System to detect asset loading failures and provide diagnostic information
/// Reports loading progress and fails fast when assets are missing
pub fn detect_asset_load_failures(
    query: Query<(&MapLoader, &MapRequestLoader)>,
    asset_server: Res<AssetServer>,
) {
    use bevy::asset::LoadState;

    for (map_loader, map_request) in query.iter() {
        // Check ground asset state with detailed reporting
        match asset_server.load_state(&map_loader.ground) {
            LoadState::Failed(err) => {
                panic!(
                    "Failed to load ground asset (.gnd) for map '{}': {:?}. File not found in GRF or data folder.",
                    map_request.map_name, err
                );
            }
            LoadState::Loading => {
                debug!("Loading ground asset for '{}'...", map_request.map_name);
            }
            LoadState::Loaded => {
                debug!("Ground asset loaded for '{}'", map_request.map_name);
            }
            LoadState::NotLoaded => {
                debug!(
                    "Ground asset not yet loading for '{}'",
                    map_request.map_name
                );
            }
        }

        // Check altitude asset if present
        if let Some(ref alt_handle) = map_loader.altitude {
            match asset_server.load_state(alt_handle) {
                LoadState::Failed(err) => {
                    panic!(
                        "Failed to load altitude asset (.gat) for map '{}': {:?}. File not found in GRF or data folder.",
                        map_request.map_name, err
                    );
                }
                LoadState::Loading => {
                    debug!("Loading altitude asset for '{}'...", map_request.map_name);
                }
                LoadState::Loaded => {
                    debug!("Altitude asset loaded for '{}'", map_request.map_name);
                }
                LoadState::NotLoaded => {
                    debug!(
                        "Altitude asset not yet loading for '{}'",
                        map_request.map_name
                    );
                }
            }
        }

        // Check world asset if present
        if let Some(ref world_handle) = map_loader.world {
            match asset_server.load_state(world_handle) {
                LoadState::Failed(err) => {
                    panic!(
                        "Failed to load world asset (.rsw) for map '{}': {:?}. File not found in GRF or data folder.",
                        map_request.map_name, err
                    );
                }
                LoadState::Loading => {
                    debug!("Loading world asset for '{}'...", map_request.map_name);
                }
                LoadState::Loaded => {
                    debug!("World asset loaded for '{}'", map_request.map_name);
                }
                LoadState::NotLoaded => {
                    debug!("World asset not yet loading for '{}'", map_request.map_name);
                }
            }
        }
    }
}
