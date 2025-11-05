use crate::{
    domain::world::components::MapLoader,
    infrastructure::assets::loaders::{RoAltitudeAsset, RoGroundAsset},
    utils::coordinates::world_position_to_spawn_coords,
};
use bevy::{math::primitives::InfinitePlane3d, prelude::*};

use super::ForwardedCursorPosition;

#[derive(Resource, Default)]
pub struct TerrainRaycastCache {
    pub cell_coords: Option<(u16, u16)>,
    pub world_position: Option<Vec3>,
    pub is_walkable: bool,
}

pub fn update_terrain_raycast_cache(
    mut cache: ResMut<TerrainRaycastCache>,
    cursor_pos: Res<ForwardedCursorPosition>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    map_loader_query: Query<&MapLoader>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    altitude_assets: Res<Assets<RoAltitudeAsset>>,
) {
    let Some(cursor_position) = cursor_pos.position else {
        cache.cell_coords = None;
        cache.world_position = None;
        cache.is_walkable = false;
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        cache.cell_coords = None;
        cache.world_position = None;
        cache.is_walkable = false;
        return;
    };

    let Ok(map_loader) = map_loader_query.single() else {
        cache.cell_coords = None;
        cache.world_position = None;
        cache.is_walkable = false;
        return;
    };

    let Some(ground_asset) = ground_assets.get(&map_loader.ground) else {
        cache.cell_coords = None;
        cache.world_position = None;
        cache.is_walkable = false;
        return;
    };

    let Some(altitude_handle) = map_loader.altitude.as_ref() else {
        cache.cell_coords = None;
        cache.world_position = None;
        cache.is_walkable = false;
        return;
    };

    let Some(altitude_asset) = altitude_assets.get(altitude_handle) else {
        cache.cell_coords = None;
        cache.world_position = None;
        cache.is_walkable = false;
        return;
    };

    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) else {
        cache.cell_coords = None;
        cache.world_position = None;
        cache.is_walkable = false;
        return;
    };

    let ground_plane = InfinitePlane3d::new(Vec3::Y);
    let plane_normal = Vec3::Y;

    let denominator = ray.direction.dot(plane_normal);
    if denominator.abs() < 1e-6 {
        cache.cell_coords = None;
        cache.world_position = None;
        cache.is_walkable = false;
        return;
    };

    let Some(initial_distance) = ray.intersect_plane(Vec3::ZERO, ground_plane) else {
        cache.cell_coords = None;
        cache.world_position = None;
        cache.is_walkable = false;
        return;
    };

    let mut world_pos = ray.origin + ray.direction * initial_distance;

    const MAX_ITERATIONS: u32 = 5;
    const CONVERGENCE_THRESHOLD: f32 = 0.1;

    for _ in 0..MAX_ITERATIONS {
        let Some(terrain_height) = altitude_asset
            .altitude
            .get_terrain_height_at_position(world_pos)
        else {
            cache.cell_coords = None;
            cache.world_position = None;
            cache.is_walkable = false;
            return;
        };

        let height_diff = (world_pos.y - terrain_height).abs();
        if height_diff < CONVERGENCE_THRESHOLD {
            world_pos.y = terrain_height;
            break;
        }

        let plane_point = Vec3::new(0.0, terrain_height, 0.0);
        let distance = (plane_point - ray.origin).dot(plane_normal) / denominator;
        world_pos = ray.origin + ray.direction * distance;
    }

    let (cell_x, cell_y) = world_position_to_spawn_coords(
        world_pos,
        ground_asset.ground.width,
        ground_asset.ground.height,
    );

    let is_walkable = altitude_asset
        .altitude
        .is_walkable(cell_x as usize, cell_y as usize);

    cache.cell_coords = Some((cell_x, cell_y));
    cache.world_position = Some(world_pos);
    cache.is_walkable = is_walkable;
}
