//! Ground-targeting AoE preview: a pool of translucent cell-snapped quads shown
//! under the cursor while a ground skill is armed (`TargetingMode::AwaitingGround`).
//!
//! The affected footprint is the server-authoritative `splash_radius` (a
//! Chebyshev square) looked up per skill from `SkillTreeState`. The pool shares a
//! single mesh + material handle and only rebuilds when the armed skill or the
//! hovered cell changes; it hides when the mode leaves `AwaitingGround`, when the
//! cursor is off-map, and despawns on `OnExit(GameState::InGame)`.
//!
//! Read-only over targeting state: it never reads or clears `ForwardedMouseClick`,
//! so the click-consumption order is untouched.

use super::VfxSystems;
use crate::core::state::GameState;
use crate::domain::input::targeting::TargetingMode;
use crate::domain::input::terrain_raycast::TerrainRaycastCache;
use crate::domain::skill::state::SkillTreeState;
use crate::domain::world::components::MapLoader;
use crate::infrastructure::assets::loaders::RoAltitudeAsset;
use crate::utils::coordinates::spawn_coords_to_world_position;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

/// Side length of a single preview quad in world units. One GAT cell spans 5.0
/// units (`RO_UNITS_PER_CELL`); the inset leaves a thin gap so the footprint
/// reads as a grid of cells rather than one solid slab.
const QUAD_SIZE: f32 = 4.6;

/// Vertical lift off the terrain surface. Up is `-Y` in this world, so a negative
/// offset raises the quad to dodge z-fighting with the ground (mirrors
/// `cast_circle.rs`'s `CIRCLE_LIFT`).
const PREVIEW_LIFT: f32 = -0.05;

/// One quad in the preview pool.
#[derive(Component)]
struct AoePreviewQuad;

/// Shared mesh + material for every pool quad, built once. The quad lies flat in
/// the XZ plane (`Rectangle` starts in XY, rotated onto the ground like the cast
/// circle's annulus).
#[derive(Resource)]
struct AoePreviewAssets {
    quad: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

impl FromWorld for AoePreviewAssets {
    fn from_world(world: &mut World) -> Self {
        let quad = world.resource_mut::<Assets<Mesh>>().add(
            Mesh::from(Rectangle::new(QUAD_SIZE, QUAD_SIZE).mesh())
                .rotated_by(Quat::from_rotation_x(FRAC_PI_2)),
        );
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(preview_material());
        Self { quad, material }
    }
}

fn preview_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(0.4, 1.0, 0.5, 0.35),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    }
}

/// Last rebuilt `(skill_id, hovered cell)`. The pool is repositioned only when
/// this key changes; `None` means the pool is currently hidden.
#[derive(Resource, Default)]
struct AoePreviewKey(Option<(u32, (u16, u16))>);

/// Cells covered by a `radius`-Chebyshev square centered on `center`, clamped to
/// the GAT `dims` (out-of-bounds cells are simply absent). Radius 0 yields the
/// single center cell.
pub fn affected_cells(center: (u16, u16), radius: u16, dims: (u16, u16)) -> Vec<(u16, u16)> {
    let (cx, cy) = center;
    let (w, h) = dims;
    if w == 0 || h == 0 {
        return Vec::new();
    }

    let min_x = cx.saturating_sub(radius);
    let max_x = cx.saturating_add(radius).min(w - 1);
    let min_y = cy.saturating_sub(radius);
    let max_y = cy.saturating_add(radius).min(h - 1);

    let mut cells = Vec::new();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            cells.push((x, y));
        }
    }
    cells
}

/// Rebuild the quad pool when the armed skill or hovered cell changes. Hidden
/// (pool despawned, key cleared) whenever the mode is not `AwaitingGround` or the
/// cursor is off-map. A skill missing from the tree or with radius 0 previews a
/// single cell.
#[allow(clippy::too_many_arguments)]
fn update_aoe_preview(
    targeting: Res<TargetingMode>,
    cache: Res<TerrainRaycastCache>,
    tree: Res<SkillTreeState>,
    assets: Res<AoePreviewAssets>,
    map_loader_query: Query<&MapLoader>,
    altitude_assets: Res<Assets<RoAltitudeAsset>>,
    existing: Query<Entity, With<AoePreviewQuad>>,
    mut key: ResMut<AoePreviewKey>,
    mut commands: Commands,
) {
    let desired = match *targeting {
        TargetingMode::AwaitingGround { skill_id, .. } => {
            cache.cell_coords.map(|cell| (skill_id, cell))
        }
        _ => None,
    };

    if desired == key.0 {
        return;
    }

    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let Some((skill_id, cell)) = desired else {
        key.0 = None;
        return;
    };

    let Some(altitude) = map_loader_query
        .single()
        .ok()
        .and_then(|loader| loader.altitude.as_ref())
        .and_then(|handle| altitude_assets.get(handle))
    else {
        // Terrain not resolved yet; stay hidden and retry next frame.
        key.0 = None;
        return;
    };

    let radius = tree
        .skills
        .get(&skill_id)
        .map_or(0, |node| node.splash_radius);
    let dims = (
        altitude.altitude.width as u16,
        altitude.altitude.height as u16,
    );

    for (cx, cy) in affected_cells(cell, radius, dims) {
        let world = spawn_coords_to_world_position(cx, cy, 0, 0);
        let Some(height) = altitude.altitude.get_terrain_height_at_position(world) else {
            continue;
        };
        commands.spawn((
            Mesh3d(assets.quad.clone()),
            MeshMaterial3d(assets.material.clone()),
            Transform::from_xyz(world.x, height + PREVIEW_LIFT, world.z),
            NotShadowCaster,
            AoePreviewQuad,
        ));
    }

    key.0 = desired;
}

fn despawn_aoe_preview(
    existing: Query<Entity, With<AoePreviewQuad>>,
    mut key: ResMut<AoePreviewKey>,
    mut commands: Commands,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    key.0 = None;
}

pub struct AoePreviewPlugin;

impl Plugin for AoePreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AoePreviewAssets>()
            .init_resource::<AoePreviewKey>()
            .add_systems(
                Update,
                update_aoe_preview
                    .in_set(VfxSystems)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(OnExit(GameState::InGame), despawn_aoe_preview);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::skill::state::SkillNode;

    #[test]
    fn radius_zero_is_a_single_cell() {
        assert_eq!(affected_cells((5, 5), 0, (100, 100)), vec![(5, 5)]);
    }

    #[test]
    fn radius_two_covers_a_five_by_five_square() {
        let cells = affected_cells((5, 5), 2, (100, 100));
        assert_eq!(cells.len(), 25);
        assert!(cells.contains(&(3, 3)));
        assert!(cells.contains(&(7, 7)));
        assert!(!cells.contains(&(8, 5)));
    }

    #[test]
    fn square_clamps_at_the_origin_corner() {
        let cells = affected_cells((0, 0), 2, (100, 100));
        assert_eq!(cells.len(), 9);
        assert!(cells.contains(&(0, 0)));
        assert!(cells.contains(&(2, 2)));
        assert!(cells.iter().all(|&(x, y)| x <= 2 && y <= 2));
    }

    #[test]
    fn square_clamps_at_the_far_corner() {
        let cells = affected_cells((99, 99), 2, (100, 100));
        assert_eq!(cells.len(), 9);
        assert!(cells.contains(&(99, 99)));
        assert!(cells.iter().all(|&(x, y)| x < 100 && y < 100));
    }

    fn preview_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<RoAltitudeAsset>()
            .init_resource::<AoePreviewAssets>()
            .init_resource::<AoePreviewKey>()
            .init_resource::<TargetingMode>()
            .init_resource::<TerrainRaycastCache>()
            .init_resource::<SkillTreeState>()
            .add_systems(Update, update_aoe_preview);
        app
    }

    fn quad_count(app: &mut App) -> usize {
        app.world_mut()
            .query::<&AoePreviewQuad>()
            .iter(app.world())
            .count()
    }

    /// A flat, fully-walkable `size`x`size` GAT so `get_terrain_height_at_position`
    /// returns `Some` for every in-bounds cell.
    fn flat_altitude(size: u32) -> RoAltitudeAsset {
        use crate::infrastructure::ro_formats::{GatCell, GatCellType, RoAltitude};
        let cells = (0..size * size)
            .map(|_| GatCell {
                height: [0.0; 4],
                cell_type: GatCellType::from(0u32),
            })
            .collect();
        RoAltitudeAsset {
            altitude: RoAltitude {
                version: "1.2".to_string(),
                width: size,
                height: size,
                cells,
            },
        }
    }

    /// Install a flat GAT + `MapLoader` and arm a ground cast hovering `cell`.
    fn arm_ground_over(app: &mut App, skill_id: u32, cell: (u16, u16)) {
        let handle = app
            .world_mut()
            .resource_mut::<Assets<RoAltitudeAsset>>()
            .add(flat_altitude(20));
        app.world_mut().spawn(MapLoader {
            ground: Handle::default(),
            altitude: Some(handle),
            world: None,
        });
        app.world_mut()
            .resource_mut::<TerrainRaycastCache>()
            .cell_coords = Some(cell);
        *app.world_mut().resource_mut::<TargetingMode>() =
            TargetingMode::AwaitingGround { skill_id, level: 5 };
    }

    #[test]
    fn ground_cast_spawns_a_cell_pool_sized_to_splash_radius() {
        let mut app = preview_app();

        // Skill absent from the tree -> radius 0 -> single-cell preview, no panic.
        arm_ground_over(&mut app, 999, (5, 5));
        app.update();
        assert_eq!(quad_count(&mut app), 1, "missing skill previews one cell");

        // A radius-2 skill covers its full 5x5 Chebyshev square.
        app.world_mut()
            .resource_mut::<SkillTreeState>()
            .skills
            .insert(
                42,
                SkillNode {
                    level: 5,
                    max_level: 5,
                    upgradable: false,
                    requires: Vec::new(),
                    req_base_level: 0,
                    req_job_level: 0,
                    sp: 0,
                    range: 0,
                    inf_type: 0,
                    job_id: 0,
                    splash_radius: 2,
                },
            );
        *app.world_mut().resource_mut::<TargetingMode>() = TargetingMode::AwaitingGround {
            skill_id: 42,
            level: 5,
        };
        app.update();
        assert_eq!(quad_count(&mut app), 25, "radius 2 previews a 5x5 square");
    }

    #[test]
    fn leaving_ingame_despawns_the_pool_and_clears_the_key() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<RoAltitudeAsset>()
            .init_resource::<AoePreviewAssets>()
            .init_resource::<AoePreviewKey>()
            .init_resource::<TargetingMode>()
            .init_resource::<TerrainRaycastCache>()
            .init_resource::<SkillTreeState>()
            .add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<GameState>()
            .add_systems(
                Update,
                update_aoe_preview.run_if(in_state(GameState::InGame)),
            )
            .add_systems(OnExit(GameState::InGame), despawn_aoe_preview);

        arm_ground_over(&mut app, 999, (5, 5));
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        assert_eq!(quad_count(&mut app), 1, "pool exists while in game");

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::CharacterSelection);
        app.update();

        assert_eq!(quad_count(&mut app), 0, "OnExit despawned the pool");
        assert!(
            app.world().resource::<AoePreviewKey>().0.is_none(),
            "OnExit cleared the rebuild key"
        );
    }

    #[test]
    fn idle_targeting_spawns_no_quads() {
        let mut app = preview_app();
        app.update();
        assert_eq!(quad_count(&mut app), 0);
    }

    #[test]
    fn awaiting_ground_without_hovered_cell_spawns_no_quads() {
        let mut app = preview_app();
        *app.world_mut().resource_mut::<TargetingMode>() = TargetingMode::AwaitingGround {
            skill_id: 83,
            level: 5,
        };
        app.update();
        assert_eq!(quad_count(&mut app), 0);
    }
}
