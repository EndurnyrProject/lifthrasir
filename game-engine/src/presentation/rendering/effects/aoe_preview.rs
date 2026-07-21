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
//!
//! During the cast itself this plugin also renders a world-anchored, element-tinted
//! spinning `Annulus` at the server-provided target cell, sized to the same
//! `splash_radius`. It spawns only for the local player's ground casts
//! (`SkillCastStarted` with `cast_time > 0`, `target_id == 0`) and mirrors the
//! caster cast-circle lifecycle (timer expiry / `CastCancelled`); the caster-anchored
//! circle in `cast_circle.rs` is left untouched.

use super::VfxSystems;
use super::cast_circle::{cast_circle_material, element_color};
use crate::core::state::GameState;
use crate::domain::entities::markers::LocalPlayer;
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::input::targeting::TargetingMode;
use crate::domain::input::terrain_raycast::TerrainRaycastCache;
use crate::domain::skill::state::SkillTreeState;
use crate::domain::world::components::MapLoader;
use crate::infrastructure::assets::loaders::RoAltitudeAsset;
use crate::utils::coordinates::spawn_coords_to_world_position;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use net_contract::events::{CastCancelled, SkillCastStarted};
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

/// Inner/outer radius ratio of the cast-time ring, matching `cast_circle.rs`'s
/// `4.5 / 6.0` inset.
const RING_INNER_RATIO: f32 = 4.5 / 6.0;

/// Radians/sec the target-area ring spins (mirrors `cast_circle.rs`).
const RING_SPIN_RATE: f32 = 1.5;

/// Outer radius of the target-area ring in world units: the ring circumscribes the
/// `splash_radius`-Chebyshev square (`(2r+1)` cells wide, 5.0 units/cell), so its
/// half-extent is `(r + 0.5) * 5.0`.
fn ring_outer_radius(splash_radius: u16) -> f32 {
    (splash_radius as f32 + 0.5) * 5.0
}

/// A world-anchored target-area ring, keyed to the caster's server id so a
/// `CastCancelled` (which carries only the gid) can find it, and a fresh cast for
/// the same caster replaces the old one.
#[derive(Component)]
struct CastAreaRing {
    caster_gid: u32,
    timer: Timer,
}

/// Spawn a target-area ring on `SkillCastStarted` for the local player's ground
/// casts (`cast_time > 0`, `target_id == 0`). The ring is anchored to the target
/// cell `(x, y)`, not the caster; outer radius `(splash_radius + 0.5) * 5.0` so it
/// circumscribes the affected Chebyshev square.
#[allow(clippy::too_many_arguments)]
fn spawn_area_rings(
    mut events: MessageReader<SkillCastStarted>,
    registry: Res<EntityRegistry>,
    locals: Query<(), With<LocalPlayer>>,
    tree: Res<SkillTreeState>,
    map_loader_query: Query<&MapLoader>,
    altitude_assets: Res<Assets<RoAltitudeAsset>>,
    existing: Query<(Entity, &CastAreaRing)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    for event in events.read() {
        if event.cast_time == 0 || event.target_id != 0 {
            continue;
        }
        let Some(caster) = registry.get_entity(event.src_id) else {
            continue;
        };
        if locals.get(caster).is_err() {
            continue;
        }

        let Some(altitude) = map_loader_query
            .single()
            .ok()
            .and_then(|loader| loader.altitude.as_ref())
            .and_then(|handle| altitude_assets.get(handle))
        else {
            continue;
        };

        let world = spawn_coords_to_world_position(event.x as u16, event.y as u16, 0, 0);
        let Some(height) = altitude.altitude.get_terrain_height_at_position(world) else {
            continue;
        };

        for (ring_entity, ring) in &existing {
            if ring.caster_gid == event.src_id {
                commands.entity(ring_entity).despawn();
            }
        }

        let radius = tree
            .skills
            .get(&event.skill_id)
            .map_or(0, |node| node.splash_radius);
        let outer = ring_outer_radius(radius);
        let mesh = meshes.add(
            Mesh::from(Annulus::new(outer * RING_INNER_RATIO, outer).mesh())
                .rotated_by(Quat::from_rotation_x(FRAC_PI_2)),
        );
        let material = materials.add(cast_circle_material(element_color(event.property)));

        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_xyz(world.x, height + PREVIEW_LIFT, world.z),
            NotShadowCaster,
            CastAreaRing {
                caster_gid: event.src_id,
                timer: Timer::from_seconds(event.cast_time as f32 / 1000.0, TimerMode::Once),
            },
        ));
    }
}

fn rotate_area_rings(time: Res<Time>, mut rings: Query<&mut Transform, With<CastAreaRing>>) {
    for mut transform in &mut rings {
        transform.rotate_y(RING_SPIN_RATE * time.delta_secs());
    }
}

fn expire_area_rings(
    time: Res<Time>,
    mut rings: Query<(Entity, &mut CastAreaRing)>,
    mut commands: Commands,
) {
    for (entity, mut ring) in &mut rings {
        ring.timer.tick(time.delta());
        if ring.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Early despawn on `CastCancelled`. An unknown gid matches no ring and is a
/// no-op.
fn cancel_area_rings(
    mut events: MessageReader<CastCancelled>,
    rings: Query<(Entity, &CastAreaRing)>,
    mut commands: Commands,
) {
    for event in events.read() {
        for (entity, ring) in &rings {
            if ring.caster_gid == event.gid {
                commands.entity(entity).despawn();
            }
        }
    }
}

fn despawn_area_rings(rings: Query<Entity, With<CastAreaRing>>, mut commands: Commands) {
    for entity in &rings {
        commands.entity(entity).despawn();
    }
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
            .add_systems(
                Update,
                (
                    spawn_area_rings,
                    rotate_area_rings,
                    expire_area_rings,
                    cancel_area_rings,
                )
                    .chain()
                    .in_set(VfxSystems)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                OnExit(GameState::InGame),
                (despawn_aoe_preview, despawn_area_rings),
            );
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

    fn ring_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<RoAltitudeAsset>()
            .init_resource::<EntityRegistry>()
            .init_resource::<SkillTreeState>()
            .add_message::<SkillCastStarted>()
            .add_message::<CastCancelled>()
            .add_systems(
                Update,
                (
                    spawn_area_rings,
                    rotate_area_rings,
                    expire_area_rings,
                    cancel_area_rings,
                )
                    .chain(),
            );
        let handle = app
            .world_mut()
            .resource_mut::<Assets<RoAltitudeAsset>>()
            .add(flat_altitude(20));
        app.world_mut().spawn(MapLoader {
            ground: Handle::default(),
            altitude: Some(handle),
            world: None,
        });
        app
    }

    fn register_caster(app: &mut App, gid: u32, local: bool) -> Entity {
        let mut ec = app
            .world_mut()
            .spawn((Transform::default(), Visibility::default()));
        if local {
            ec.insert(LocalPlayer);
        }
        let caster = ec.id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(gid, caster);
        caster
    }

    fn ground_cast(src_id: u32, target_id: u32, cast_time: u32) -> SkillCastStarted {
        SkillCastStarted {
            src_id,
            target_id,
            x: 5,
            y: 5,
            skill_id: 1,
            property: 3,
            cast_time,
        }
    }

    fn ring_count(app: &mut App) -> usize {
        app.world_mut()
            .query::<&CastAreaRing>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn ring_outer_radius_circumscribes_the_square() {
        assert!((ring_outer_radius(0) - 2.5).abs() < 1e-6);
        assert!((ring_outer_radius(2) - 12.5).abs() < 1e-6);
    }

    #[test]
    fn local_ground_cast_spawns_a_ring_at_the_target_cell() {
        let mut app = ring_app();
        register_caster(&mut app, 7, true);
        app.world_mut()
            .resource_mut::<SkillTreeState>()
            .skills
            .insert(
                11,
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

        // Asymmetric target cell so an x/y transposition would misplace the ring.
        app.world_mut().write_message(SkillCastStarted {
            src_id: 7,
            target_id: 0,
            x: 3,
            y: 7,
            skill_id: 11,
            property: 3,
            cast_time: 1000,
        });
        app.update();

        // Deterministic flat-GAT height sampled the same way the system does, so
        // the assertion pins the -Y lift relationship, not a hardcoded magic value.
        let expected = spawn_coords_to_world_position(3, 7, 0, 0);
        let world = app.world_mut();
        let handle = world
            .query::<&MapLoader>()
            .single(world)
            .unwrap()
            .altitude
            .clone()
            .unwrap();
        let terrain_height = world
            .resource::<Assets<RoAltitudeAsset>>()
            .get(&handle)
            .unwrap()
            .altitude
            .get_terrain_height_at_position(expected)
            .unwrap();

        let mut query = world.query_filtered::<(&Transform, &Mesh3d), With<CastAreaRing>>();
        let (transform, mesh) = query.single(world).expect("ring spawned");
        let translation = transform.translation;
        let mesh_handle = mesh.0.clone();

        assert!(
            (translation.x - expected.x).abs() < 1e-4,
            "ring x snapped to the target cell"
        );
        assert!(
            (translation.z - expected.z).abs() < 1e-4,
            "ring z snapped to the target cell"
        );
        assert!(
            (translation.y - (terrain_height + PREVIEW_LIFT)).abs() < 1e-4,
            "ring sits at the sampled terrain height plus the -Y lift"
        );
        assert!(
            world.resource::<Assets<Mesh>>().get(&mesh_handle).is_some(),
            "ring owns a real annulus mesh asset"
        );
    }

    #[test]
    fn each_ring_owns_a_distinct_mesh() {
        let mut app = ring_app();
        register_caster(&mut app, 1, true);
        register_caster(&mut app, 2, true);

        app.world_mut().write_message(ground_cast(1, 0, 1000));
        app.world_mut().write_message(ground_cast(2, 0, 1000));
        app.update();

        let world = app.world_mut();
        let mut query = world.query_filtered::<&Mesh3d, With<CastAreaRing>>();
        let handles: Vec<_> = query.iter(world).map(|mesh| mesh.0.id()).collect();
        assert_eq!(handles.len(), 2);
        assert_ne!(handles[0], handles[1], "rings do not share a mesh handle");
    }

    #[test]
    fn entity_targeted_cast_spawns_no_ring() {
        let mut app = ring_app();
        register_caster(&mut app, 7, true);

        app.world_mut().write_message(ground_cast(7, 42, 1000));
        app.update();

        assert_eq!(ring_count(&mut app), 0);
    }

    #[test]
    fn remote_player_ground_cast_spawns_no_ring() {
        let mut app = ring_app();
        register_caster(&mut app, 8, false);

        app.world_mut().write_message(ground_cast(8, 0, 1000));
        app.update();

        assert_eq!(ring_count(&mut app), 0);
    }

    #[test]
    fn cast_cancelled_despawns_the_ring() {
        let mut app = ring_app();
        register_caster(&mut app, 7, true);

        app.world_mut().write_message(ground_cast(7, 0, 5000));
        app.update();
        assert_eq!(ring_count(&mut app), 1);

        app.world_mut().write_message(CastCancelled { gid: 999 });
        app.update();
        assert_eq!(ring_count(&mut app), 1, "unknown gid is a no-op");

        app.world_mut().write_message(CastCancelled { gid: 7 });
        app.update();
        assert_eq!(ring_count(&mut app), 0, "matching gid despawns the ring");
    }
}
