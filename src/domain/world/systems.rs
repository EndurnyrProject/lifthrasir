use crate::domain::world::components::MapLoader;
use crate::domain::world::map_loader::{ExtractedMapFiles, MapRequestLoader};
use crate::infrastructure::assets::HierarchicalAssetManager;
use crate::infrastructure::assets::loaders::{RoAltitudeAsset, RoGroundAsset, RoWorldAsset};
use crate::infrastructure::ro_formats::{RoAltitude, RoGround, RoWorld};
use bevy::prelude::*;

pub fn extract_map_from_hierarchical_assets(
    mut commands: Commands,
    manager: Option<Res<HierarchicalAssetManager>>,
    mut ground_assets: ResMut<Assets<RoGroundAsset>>,
    mut altitude_assets: ResMut<Assets<RoAltitudeAsset>>,
    mut world_assets: ResMut<Assets<RoWorldAsset>>,
    mut query: Query<(Entity, &mut MapRequestLoader), Without<ExtractedMapFiles>>,
) {
    let Some(ref manager) = manager else {
        return;
    };

    for (entity, mut map_loader) in query.iter_mut() {
        if map_loader.loaded {
            continue;
        }

        let mut extracted = ExtractedMapFiles::new();
        let map_name = &map_loader.map_name;

        // Let the HierarchicalAssetManager handle path resolution
        // Use canonical RO path format - the asset manager will try different sources
        let gnd_path = format!("data\\{}.gnd", map_name);
        let gat_path = format!("data\\{}.gat", map_name);
        let rsw_path = format!("data\\{}.rsw", map_name);

        // Load .gnd file
        if let Ok(gnd_data) = manager.load(&gnd_path) {
            extracted.ground_data = Some(gnd_data);
        } else {
            warn!(
                "Failed to load ground data for map '{}' at path '{}'",
                map_name, gnd_path
            );
        }

        // Load .gat file
        if let Ok(gat_data) = manager.load(&gat_path) {
            extracted.altitude_data = Some(gat_data);
        } else {
            warn!(
                "Failed to load altitude data for map '{}' at path '{}'",
                map_name, gat_path
            );
        }

        // Load .rsw file
        if let Ok(rsw_data) = manager.load(&rsw_path) {
            extracted.world_data = Some(rsw_data);
        } else {
            warn!(
                "Failed to load world data for map '{}' at path '{}'",
                map_name, rsw_path
            );
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
        map_loader.loaded = true;
        commands.entity(entity).insert(extracted);
    }
}

pub fn setup_hierarchical_map_loading(mut commands: Commands) {
    // Request loading of a specific map through the hierarchical asset system
    commands.spawn(MapRequestLoader::new("aldebaran".to_string()));
}
