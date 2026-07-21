use crate::{
    domain::{system_sets::InputSystems, world::components::MapLoader},
    infrastructure::assets::loaders::{RoAltitudeAsset, RoGroundAsset},
    utils::coordinates::world_position_to_spawn_coords,
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
#[auto_init_resource(plugin = crate::app::input_plugin::InputPlugin)]
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
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = Update,
    config(in_set = InputSystems::Raycast)
)]
pub fn update_terrain_raycast_cache(
    mut cache: ResMut<TerrainRaycastCache>,
    cursor_pos: Res<ForwardedCursorPosition>,
    camera_query: Query<(&Camera, &GlobalTransform), GameCameraFilter>,
    map_loader_query: Query<&MapLoader>,
    ground_assets: Res<Assets<RoGroundAsset>>,
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

    let Ok(map_loader) = map_loader_query.single() else {
        cache.clear();
        return;
    };

    let Some(ground_asset) = ground_assets.get(&map_loader.ground) else {
        cache.clear();
        return;
    };

    let Some(altitude_handle) = map_loader.altitude.as_ref() else {
        cache.clear();
        return;
    };

    let Some(altitude_asset) = altitude_assets.get(altitude_handle) else {
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

    // March the ray against the terrain heightfield and bisect the surface crossing.
    // A fixed-point plane refinement diverged for shallow (near-horizon) angles, so the
    // gizmo lost the cursor near the top of the screen; marching is stable at any angle
    // and stops cleanly at the map edge (height lookup returns None off-map).
    const STEP: f32 = 2.0;
    const MAX_STEPS: u32 = 4096;
    const BISECT_STEPS: u32 = 8;

    let signed_gap = |p: Vec3| {
        altitude_asset
            .altitude
            .get_terrain_height_at_position(p)
            .map(|height| p.y - height)
    };

    let mut above = ray.origin;
    let mut above_gap = signed_gap(above);
    let mut crossing = None;
    for step in 1..=MAX_STEPS {
        let current = ray.origin + ray.direction * (step as f32 * STEP);
        let current_gap = signed_gap(current);
        if let (Some(prev), Some(cur)) = (above_gap, current_gap)
            && prev <= 0.0 && cur >= 0.0 {
                crossing = Some((above, current));
                break;
            }
        above = current;
        above_gap = current_gap;
    }

    let Some((mut lo, mut hi)) = crossing else {
        cache.cell_coords = None;
        cache.world_position = None;
        cache.is_walkable = false;
        cache.last_input = Some((cursor_position, *camera_transform));
        return;
    };

    for _ in 0..BISECT_STEPS {
        let mid = (lo + hi) * 0.5;
        match signed_gap(mid) {
            Some(gap) if gap < 0.0 => lo = mid,
            Some(_) => hi = mid,
            None => break,
        }
    }
    let world_pos = (lo + hi) * 0.5;

    let (raw_x, raw_y) = world_position_to_spawn_coords(
        world_pos,
        ground_asset.ground.width,
        ground_asset.ground.height,
    );

    // The raycast resolves one grid cell off from the cursor's true cell (-1 X, +1 Y).
    // Correct at this single source so the gizmo, walkability, and click-to-move agree.
    // No upper clamp: cells are in GAT space, while ground.{width,height} are GND-resolution
    // (half), so clamping against them froze the gizmo mid-map. is_walkable bounds-checks
    // internally and the ray-march already returns None past the true map edge.
    let cell_x = raw_x.saturating_sub(1);
    let cell_y = raw_y + 1;

    let is_walkable = altitude_asset
        .altitude
        .is_walkable(cell_x as usize, cell_y as usize);

    cache.cell_coords = Some((cell_x, cell_y));
    cache.world_position = Some(world_pos);
    cache.is_walkable = is_walkable;
    cache.last_input = Some((cursor_position, *camera_transform));
}
