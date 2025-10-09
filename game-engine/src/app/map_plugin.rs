use crate::{
    domain::assets::components::WaterMaterial,
    domain::world::components::MapLoader,
    infrastructure::assets::loaders::{RoGroundAsset, RoWorldAsset},
    infrastructure::ro_formats::RswObject,
    presentation::rendering::lighting::EnhancedLightingPlugin,
    presentation::rendering::models::{
        spawn_map_models, load_rsm_assets, update_model_meshes,
        create_model_materials_when_textures_ready, update_rsm_animations,
    },
    presentation::rendering::water::{animate_water_system, finalize_water_loading_system, load_water_system},
};
use bevy::prelude::*;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
                MaterialPlugin::<WaterMaterial>::default(),
                EnhancedLightingPlugin,
            ))
            .add_systems(
                Update,
                (
                    log_loaded_world_data,
                    spawn_map_models,
                    load_rsm_assets,
                ),
            )
            .add_systems(
                Update,
                (
                    update_model_meshes,
                    create_model_materials_when_textures_ready,
                    update_rsm_animations,
                    load_water_system,
                    finalize_water_loading_system,
                    animate_water_system,
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
                info!("  Water level: {}", world_asset.world.water.level);
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
