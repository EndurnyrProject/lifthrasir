use crate::{
    domain::{system_sets::InputSystems, world::components::CurrentMapAltitude},
    infrastructure::assets::loaders::RoAltitudeAsset,
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_system, auto_init_resource};

use super::ForwardedCursorPosition;

/// Query filter for the game 3D camera, excluding the equipment-window preview
/// camera (a 2D UI camera also exists, so `With<Camera3d>` disambiguates).
type GameCameraFilter = (
    With<Camera3d>,
    Without<crate::domain::entities::billboard::EquipmentPreviewCamera>,
);

#[derive(Resource, Default, Reflect)]
#[reflect(Resource, Default)]
#[auto_init_resource(plugin = crate::domain::input::plugin::InputPlugin)]
pub struct TerrainRaycastCache {
    pub cell_coords: Option<(u16, u16)>,
    pub world_position: Option<Vec3>,
    pub is_walkable: bool,
    /// Cursor + camera pose of the last completed march. While both are
    /// unchanged the cached result is still valid and the (expensive) ray
    /// march is skipped. Only set once the map assets resolved, so frames
    /// bailing on missing assets keep retrying.
    last_input: Option<(Vec2, GlobalTransform)>,
}

impl TerrainRaycastCache {
    fn clear(&mut self) {
        self.cell_coords = None;
        self.world_position = None;
        self.is_walkable = false;
        self.last_input = None;
    }
}

#[auto_add_system(
    plugin = crate::domain::input::plugin::InputPlugin,
    schedule = Update,
    config(in_set = InputSystems::Raycast)
)]
pub fn update_terrain_raycast_cache(
    mut cache: ResMut<TerrainRaycastCache>,
    cursor_pos: Res<ForwardedCursorPosition>,
    camera_query: Query<(&Camera, &GlobalTransform), GameCameraFilter>,
    map_altitude: Option<Res<CurrentMapAltitude>>,
    altitude_assets: Res<Assets<RoAltitudeAsset>>,
) {
    let Some(cursor_position) = cursor_pos.position else {
        cache.clear();
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        cache.clear();
        return;
    };

    if cache.last_input == Some((cursor_position, *camera_transform)) {
        return;
    }

    let Some(map_altitude) = map_altitude else {
        cache.clear();
        return;
    };

    let Some(altitude_asset) = altitude_assets.get(&map_altitude.0) else {
        cache.clear();
        return;
    };

    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) else {
        cache.clear();
        return;
    };

    if ray.direction.dot(Vec3::Y).abs() < 1e-6 {
        cache.clear();
        return;
    }

    match lifthrasir_runtime::world::terrain::raycast_terrain(&altitude_asset.altitude, ray) {
        Some(hit) => {
            cache.cell_coords = Some(hit.cell_coords);
            cache.world_position = Some(hit.world_position);
            cache.is_walkable = hit.is_walkable;
        }
        None => {
            cache.cell_coords = None;
            cache.world_position = None;
            cache.is_walkable = false;
        }
    }
    cache.last_input = Some((cursor_position, *camera_transform));
}
