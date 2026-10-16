use super::world_position_to_spawn_coords;
use bevy::prelude::*;
use ro_formats::RoAltitude;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainHit {
    pub world_position: Vec3,
    pub cell_coords: (u16, u16),
    pub is_walkable: bool,
}

/// Intersect an explicit ray with a terrain heightfield using the calibrated cursor march.
pub fn raycast_terrain(altitude: &RoAltitude, ray: Ray3d) -> Option<TerrainHit> {
    if ray.direction.dot(Vec3::Y).abs() < 1e-6 {
        return None;
    }
    // March the ray against the terrain heightfield and bisect the surface crossing.
    // A fixed-point plane refinement diverged for shallow (near-horizon) angles, so the
    // gizmo lost the cursor near the top of the screen; marching is stable at any angle
    // and stops cleanly at the map edge (height lookup returns None off-map).
    const STEP: f32 = 2.0;
    const MAX_STEPS: u32 = 4096;
    const BISECT_STEPS: u32 = 8;

    let signed_gap = |p: Vec3| {
        altitude
            .get_terrain_height_at_position(p)
            .map(|height| p.y - height)
    };

    // `signed_gap` is positive above the surface; the camera starts above and
    // the crossing is the first step that lands on or below it.
    let mut above = ray.origin;
    let mut above_gap = signed_gap(above);
    let mut crossing = None;
    for step in 1..=MAX_STEPS {
        let current = ray.origin + ray.direction * (step as f32 * STEP);
        let current_gap = signed_gap(current);
        if let (Some(prev), Some(cur)) = (above_gap, current_gap)
            && prev >= 0.0
            && cur <= 0.0
        {
            crossing = Some((above, current));
            break;
        }
        above = current;
        above_gap = current_gap;
    }

    let (mut lo, mut hi) = crossing?;

    for _ in 0..BISECT_STEPS {
        let mid = (lo + hi) * 0.5;
        match signed_gap(mid) {
            Some(gap) if gap > 0.0 => lo = mid,
            Some(_) => hi = mid,
            None => break,
        }
    }
    let world_pos = (lo + hi) * 0.5;

    let (raw_x, raw_y) = world_position_to_spawn_coords(world_pos);

    // The raycast resolves one grid cell off from the cursor's true cell (-1 X, +1 Y).
    // Correct at this single source so the gizmo, walkability, and click-to-move agree.
    // No upper clamp: cells are in GAT space, so clamping against the GND-resolution
    // (half) map dimensions froze the gizmo mid-map. is_walkable bounds-checks internally
    // and the ray-march already returns None past the true map edge.
    let cell_x = raw_x.saturating_sub(1);
    let cell_y = raw_y + 1;

    let is_walkable = altitude.is_walkable(cell_x as usize, cell_y as usize);

    Some(TerrainHit {
        world_position: world_pos,
        cell_coords: (cell_x, cell_y),
        is_walkable,
    })
}
