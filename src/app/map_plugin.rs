use crate::{
    assets::loaders::{RoGroundAsset, RoWorldAsset},
    components::MapLoader,
    ro_formats::RswObject,
    systems::{
        RsmCache, generate_terrain_mesh, setup_terrain_camera, spawn_map_models,
        update_model_meshes, update_rsm_animations,
    },
};
use bevy::prelude::*;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RsmCache>().add_systems(
            Update,
            (
                log_loaded_world_data,
                generate_terrain_mesh,
                setup_terrain_camera,
                spawn_map_models,
                update_model_meshes,
                update_rsm_animations,
            ),
        );
    }
}

fn log_loaded_world_data(
    world_assets: Res<Assets<RoWorldAsset>>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    query: Query<&MapLoader, Changed<MapLoader>>,
) {
    for map_loader in query.iter() {
        if let Some(world) = map_loader.world.as_ref() {
            if let Some(world_asset) = world_assets.get(world) {
                let model_count = world_asset
                    .world
                    .objects
                    .iter()
                    .filter(|o| matches!(o, RswObject::Model(_)))
                    .count();
                let light_count = world_asset
                    .world
                    .objects
                    .iter()
                    .filter(|o| matches!(o, RswObject::Light(_)))
                    .count();
                let sound_count = world_asset
                    .world
                    .objects
                    .iter()
                    .filter(|o| matches!(o, RswObject::Sound(_)))
                    .count();
                let effect_count = world_asset
                    .world
                    .objects
                    .iter()
                    .filter(|o| matches!(o, RswObject::Effect(_)))
                    .count();

                info!("World data loaded:");
                info!("  Version: {}", world_asset.world.version);
                info!("  GND file: {}", world_asset.world.gnd_file);
                info!("  GAT file: {}", world_asset.world.gat_file);
                info!("  Total objects: {}", world_asset.world.objects.len());
                info!("    Models: {}", model_count);
                info!("    Lights: {}", light_count);
                info!("    Sounds: {}", sound_count);
                info!("    Effects: {}", effect_count);
            }
        }

        if let Some(ground_asset) = ground_assets.get(&map_loader.ground) {
            info!("Ground data loaded:");
            info!(
                "  Size: {}x{}",
                ground_asset.ground.width, ground_asset.ground.height
            );
            info!("  Textures: {}", ground_asset.ground.textures.len());
        }
    }
}
