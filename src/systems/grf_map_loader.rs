use crate::assets::loaders::{GrfAsset, RoAltitudeAsset, RoGroundAsset, RoWorldAsset};
use crate::components::{ExtractedMapFiles, GrfMapLoader, MapLoader};
use crate::ro_formats::{RoAltitude, RoGround, RoWorld};
use bevy::prelude::*;

pub fn extract_map_from_grf(
    mut commands: Commands,
    grf_assets: Res<Assets<GrfAsset>>,
    mut ground_assets: ResMut<Assets<RoGroundAsset>>,
    mut altitude_assets: ResMut<Assets<RoAltitudeAsset>>,
    mut world_assets: ResMut<Assets<RoWorldAsset>>,
    mut query: Query<(Entity, &mut GrfMapLoader), Without<ExtractedMapFiles>>,
    asset_server: Res<AssetServer>,
) {
    for (entity, mut grf_loader) in query.iter_mut() {
        if grf_loader.loaded {
            continue;
        }

        if let Some(grf_asset) = grf_assets.get(&grf_loader.grf_handle) {
            let mut extracted = ExtractedMapFiles::new();
            let map_name = &grf_loader.map_name;

            // Follow roBrowser's approach: use backslashes for GRF paths
            let possible_paths = vec![
                format!("data\\{}.gnd", map_name),
                format!("data\\{}.gat", map_name),
                format!("data\\{}.rsw", map_name),
                // Also try forward slashes as fallback
                format!("data/{}.gnd", map_name),
                format!("data/{}.gat", map_name),
                format!("data/{}.rsw", map_name),
                // Try without data/ prefix
                format!("{}.gnd", map_name),
                format!("{}.gat", map_name),
                format!("{}.rsw", map_name),
            ];

            // Extract map files
            for path in &[&possible_paths[0], &possible_paths[3]] {
                if let Some(gnd_data) = grf_asset.grf.get_file(path) {
                    extracted.ground_data = Some(gnd_data);
                    break;
                }
            }

            for path in &[&possible_paths[1], &possible_paths[4]] {
                if let Some(gat_data) = grf_asset.grf.get_file(path) {
                    extracted.altitude_data = Some(gat_data);
                    break;
                }
            }

            for path in &[&possible_paths[2], &possible_paths[5]] {
                if let Some(rsw_data) = grf_asset.grf.get_file(path) {
                    extracted.world_data = Some(rsw_data);
                    break;
                }
            }

            // Convert extracted data to Bevy assets
            let mut ground_handle = None;
            let mut altitude_handle = None;
            let mut world_handle = None;

            if let Some(gnd_data) = &extracted.ground_data {
                match RoGround::from_bytes(gnd_data) {
                    Ok(ground) => {
                        let asset = RoGroundAsset { ground };
                        ground_handle = Some(ground_assets.add(asset));
                    }
                    Err(e) => error!("Failed to parse GND data: {}", e),
                }
            }

            if let Some(gat_data) = &extracted.altitude_data {
                match RoAltitude::from_bytes(gat_data) {
                    Ok(altitude) => {
                        let asset = RoAltitudeAsset { altitude };
                        altitude_handle = Some(altitude_assets.add(asset));
                    }
                    Err(e) => error!("Failed to parse GAT data: {}", e),
                }
            }

            if let Some(rsw_data) = &extracted.world_data {
                match RoWorld::from_bytes(rsw_data) {
                    Ok(world) => {
                        let asset = RoWorldAsset { world };
                        world_handle = Some(world_assets.add(asset));
                    }
                    Err(e) => error!("Failed to parse RSW data: {}", e),
                }
            }

            // Create MapLoader with the extracted assets
            if let Some(ground) = ground_handle {
                commands.entity(entity).insert(MapLoader {
                    ground,
                    altitude: altitude_handle,
                    world: world_handle,
                });
            } else {
                error!("Failed to extract ground data for map '{}'", map_name);
            }

            // Mark as loaded and add extracted data component
            grf_loader.loaded = true;
            commands.entity(entity).insert(extracted);
        }
    }
}

pub fn setup_grf_map_loading(mut commands: Commands, asset_server: Res<AssetServer>) {
    let grf_handle: Handle<GrfAsset> = asset_server.load("20250416Ragnarok_en/data.grf");
    commands.spawn(GrfMapLoader::new(grf_handle, "aldebaran".to_string()));
}
