//! Casting magic circle: a rotating, element-tinted ring under a caster while
//! `SkillCastStarted.cast_time` runs. Entity-anchored (child of the caster) so it
//! rides the caster's own transform for free; despawns on timer expiry or an
//! early `CastCancelled` for the same caster.
//!
//! GRF has no reusable neutral ring texture (the `ring`/`circle` hits under
//! `data\texture\effect\` are per-skill numbered animation frames, e.g. the
//! `4ig_firstfaithpower`/`abyss_chaser` sets, not a generic tintable asset). So
//! the ring is procedural: a flat `Annulus` mesh, tinted per element, following
//! the same procedural-quad pattern as `portal.rs`.

use super::VfxSystems;
use crate::domain::entities::registry::EntityRegistry;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use net_contract::events::{CastCancelled, SkillCastStarted};
use std::f32::consts::FRAC_PI_2;

/// Ring inner/outer radius in world units, sized to sit just under a
/// character's footprint (`CELL_SIZE` = 10.0).
const OUTER_RADIUS: f32 = 6.0;
const INNER_RADIUS: f32 = 4.5;

/// Vertical offset from the caster's origin. Up is `-Y` in this world, so a
/// negative offset lifts the ring off the ground plane to dodge z-fighting.
const CIRCLE_LIFT: f32 = -0.05;

/// Radians/sec the ring spins around its local Y axis.
const SPIN_RATE: f32 = 1.5;

/// aesir property (element) enum, 0-9. Unknown/out-of-range values fall back to
/// neutral.
const ELEMENT_COLORS: [Color; 10] = [
    Color::srgb(1.0, 1.0, 1.0),    // 0 neutral
    Color::srgb(0.25, 0.55, 1.0),  // 1 water
    Color::srgb(0.55, 0.4, 0.15),  // 2 earth
    Color::srgb(1.0, 0.35, 0.1),   // 3 fire
    Color::srgb(0.75, 0.9, 0.25),  // 4 wind
    Color::srgb(0.55, 0.15, 0.75), // 5 poison
    Color::srgb(1.0, 0.9, 0.6),    // 6 holy
    Color::srgb(0.25, 0.05, 0.35), // 7 shadow
    Color::srgb(0.75, 0.75, 0.8),  // 8 ghost
    Color::srgb(0.45, 0.55, 0.15), // 9 undead
];

pub(super) fn element_color(property: u32) -> Color {
    ELEMENT_COLORS
        .get(property as usize)
        .copied()
        .unwrap_or(ELEMENT_COLORS[0])
}

/// Marks the ring entity and keys it to the caster's server id, so a
/// `CastCancelled` (which carries only the gid) can find it without a parent
/// lookup, and a fresh cast for the same caster can replace the old one.
#[derive(Component)]
struct CastCircle {
    caster_gid: u32,
    timer: Timer,
}

/// Shared ring mesh, built once. Lies flat in the XZ plane with a `-Y`-facing
/// normal (matches the pick-plane convention: this world's default culling
/// backface-culls a `+Y`-facing plane).
#[derive(Resource)]
struct CastCircleAssets {
    ring: Handle<Mesh>,
}

impl FromWorld for CastCircleAssets {
    fn from_world(world: &mut World) -> Self {
        let ring = world.resource_mut::<Assets<Mesh>>().add(
            Mesh::from(Annulus::new(INNER_RADIUS, OUTER_RADIUS).mesh())
                .rotated_by(Quat::from_rotation_x(FRAC_PI_2)),
        );
        Self { ring }
    }
}

pub(super) fn cast_circle_material(color: Color) -> StandardMaterial {
    let c = color.to_srgba();
    StandardMaterial {
        base_color: Color::srgba(c.red, c.green, c.blue, 0.65),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    }
}

/// Spawn a ring on `SkillCastStarted` with `cast_time > 0`. One circle per
/// caster: a new cast for the same caster despawns the previous ring first.
fn spawn_cast_circles(
    mut events: MessageReader<SkillCastStarted>,
    mut commands: Commands,
    registry: Res<EntityRegistry>,
    assets: Res<CastCircleAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<(Entity, &CastCircle)>,
) {
    for event in events.read() {
        if event.cast_time == 0 {
            continue;
        }
        let Some(caster) = registry.get_entity(event.src_id) else {
            continue;
        };

        for (circle_entity, circle) in &existing {
            if circle.caster_gid == event.src_id {
                commands.entity(circle_entity).despawn();
            }
        }

        let material = materials.add(cast_circle_material(element_color(event.property)));
        commands.entity(caster).with_children(|parent| {
            parent.spawn((
                Mesh3d(assets.ring.clone()),
                MeshMaterial3d(material),
                Transform::from_xyz(0.0, CIRCLE_LIFT, 0.0),
                NotShadowCaster,
                CastCircle {
                    caster_gid: event.src_id,
                    timer: Timer::from_seconds(event.cast_time as f32 / 1000.0, TimerMode::Once),
                },
            ));
        });
    }
}

fn rotate_cast_circles(time: Res<Time>, mut circles: Query<&mut Transform, With<CastCircle>>) {
    for mut transform in &mut circles {
        transform.rotate_y(SPIN_RATE * time.delta_secs());
    }
}

fn expire_cast_circles(
    time: Res<Time>,
    mut circles: Query<(Entity, &mut CastCircle)>,
    mut commands: Commands,
) {
    for (entity, mut circle) in &mut circles {
        circle.timer.tick(time.delta());
        if circle.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Early despawn on `CastCancelled`. An unknown gid matches no ring and is a
/// no-op.
fn cancel_cast_circles(
    mut events: MessageReader<CastCancelled>,
    circles: Query<(Entity, &CastCircle)>,
    mut commands: Commands,
) {
    for event in events.read() {
        for (entity, circle) in &circles {
            if circle.caster_gid == event.gid {
                commands.entity(entity).despawn();
            }
        }
    }
}

pub struct CastCircleVfxPlugin;

impl Plugin for CastCircleVfxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CastCircleAssets>().add_systems(
            Update,
            (
                spawn_cast_circles,
                rotate_cast_circles,
                expire_cast_circles,
                cancel_cast_circles,
            )
                .chain()
                .in_set(VfxSystems),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_resource::<CastCircleAssets>()
            .init_resource::<EntityRegistry>()
            .add_message::<SkillCastStarted>()
            .add_message::<CastCancelled>()
            .add_systems(
                Update,
                (
                    spawn_cast_circles,
                    rotate_cast_circles,
                    expire_cast_circles,
                    cancel_cast_circles,
                )
                    .chain(),
            );
        app
    }

    fn spawn_caster(app: &mut App, gid: u32) -> Entity {
        let caster = app
            .world_mut()
            .spawn((Transform::default(), Visibility::default()))
            .id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(gid, caster);
        caster
    }

    fn cast_started(src_id: u32, property: u32, cast_time: u32) -> SkillCastStarted {
        SkillCastStarted {
            src_id,
            target_id: 0,
            x: 0,
            y: 0,
            skill_id: 1,
            property,
            cast_time,
        }
    }

    fn circle_count(app: &mut App) -> usize {
        app.world_mut()
            .query::<&CastCircle>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn cast_started_spawns_tinted_ring_child() {
        let mut app = test_app();
        let caster = spawn_caster(&mut app, 7);

        app.world_mut().write_message(cast_started(7, 3, 1000)); // fire
        app.update();

        let world = app.world_mut();
        let (entity, circle) = world
            .query::<(Entity, &CastCircle)>()
            .single(world)
            .expect("ring spawned");
        assert_eq!(circle.caster_gid, 7);
        assert_eq!(world.get::<ChildOf>(entity).unwrap().parent(), caster);

        let handle = world
            .get::<MeshMaterial3d<StandardMaterial>>(entity)
            .unwrap();
        let material = world
            .resource::<Assets<StandardMaterial>>()
            .get(&handle.0)
            .unwrap();
        let expected = element_color(3).to_srgba();
        let actual = material.base_color.to_srgba();
        assert!((actual.red - expected.red).abs() < 1e-4);
        assert!((actual.green - expected.green).abs() < 1e-4);
        assert!((actual.blue - expected.blue).abs() < 1e-4);
    }

    #[test]
    fn neutral_property_tints_white() {
        let mut app = test_app();
        spawn_caster(&mut app, 9);

        app.world_mut().write_message(cast_started(9, 0, 500));
        app.update();

        let world = app.world_mut();
        let (entity, _) = world
            .query::<(Entity, &CastCircle)>()
            .single(world)
            .expect("ring spawned");
        let handle = world
            .get::<MeshMaterial3d<StandardMaterial>>(entity)
            .unwrap();
        let material = world
            .resource::<Assets<StandardMaterial>>()
            .get(&handle.0)
            .unwrap();
        let actual = material.base_color.to_srgba();
        assert!((actual.red - 1.0).abs() < 1e-4);
        assert!((actual.green - 1.0).abs() < 1e-4);
        assert!((actual.blue - 1.0).abs() < 1e-4);
    }

    #[test]
    fn zero_cast_time_spawns_no_circle() {
        let mut app = test_app();
        spawn_caster(&mut app, 3);

        app.world_mut().write_message(cast_started(3, 1, 0));
        app.update();

        assert_eq!(circle_count(&mut app), 0);
    }

    #[test]
    fn timer_elapsing_despawns_the_circle() {
        let mut app = test_app();
        spawn_caster(&mut app, 4);

        app.world_mut().write_message(cast_started(4, 1, 300));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        app.update();
        assert_eq!(circle_count(&mut app), 1, "ring exists mid-cast");

        // Advance past the 300ms cast in sub-max_delta chunks (Bevy clamps a
        // single frame's delta).
        for _ in 0..2 {
            app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                200,
            )));
            app.update();
        }
        assert_eq!(circle_count(&mut app), 0, "ring despawns after cast_time");
    }

    #[test]
    fn cast_cancelled_despawns_early_unknown_gid_is_noop() {
        let mut app = test_app();
        spawn_caster(&mut app, 5);

        app.world_mut().write_message(cast_started(5, 1, 5000));
        app.update();
        assert_eq!(circle_count(&mut app), 1);

        app.world_mut().write_message(CastCancelled { gid: 999 });
        app.update();
        assert_eq!(circle_count(&mut app), 1, "unknown gid is a no-op");

        app.world_mut().write_message(CastCancelled { gid: 5 });
        app.update();
        assert_eq!(circle_count(&mut app), 0, "matching gid despawns early");
    }

    #[test]
    fn new_cast_for_same_caster_replaces_the_old_ring() {
        let mut app = test_app();
        spawn_caster(&mut app, 6);

        app.world_mut().write_message(cast_started(6, 1, 5000));
        app.update();
        assert_eq!(circle_count(&mut app), 1);

        app.world_mut().write_message(cast_started(6, 3, 5000));
        app.update();
        assert_eq!(circle_count(&mut app), 1, "replaced, not stacked");
    }
}
