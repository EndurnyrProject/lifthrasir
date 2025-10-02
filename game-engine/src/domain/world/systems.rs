use crate::domain::world::components::MapLoader;
use crate::domain::world::map_loader::MapRequestLoader;
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

        // Use unified asset source with ro:// prefix
        let gnd_path = format!("ro://data/{}.gnd", map_name);
        let gat_path = format!("ro://data/{}.gat", map_name);
        let rsw_path = format!("ro://data/{}.rsw", map_name);

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

        // Mark as loaded - AssetServer handles the actual loading asynchronously
        map_loader.loaded = true;
    }
}

pub fn setup_unified_map_loading(mut commands: Commands) {
    // Request loading of a specific map through the unified asset system
    commands.spawn(MapRequestLoader::new("aldebaran".to_string()));
}
