//! The ring at the feet of the locked attack target: a pulsing red ellipse that
//! follows the unit for as long as the lock holds.
//!
//! NOTE: the ring is `bevy_ui`, so it draws over the sprite's feet rather than
//! under them. Move it to a ground decal if that reads wrong in play.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::screens::shared::glow;
use crate::worldspace::{LockedUnit, WorldAnchor, WorldOverlaySystems, hung_root};

const RING_SIZE: Vec2 = Vec2::new(58.0, 14.0);
const RING_COLOR: Color = Color::srgb_u8(0xd0, 0x54, 0x4a);
/// One full fade down to half opacity and back.
const PULSE_SECS: f32 = 1.2;
/// Under the name plates (+100); see [`super::WORLD_OVERLAY_Z`].
const RING_Z: i32 = super::WORLD_OVERLAY_Z + 90;

pub struct TargetRingPlugin;

impl Plugin for TargetRingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (sync_ring, pulse_ring)
                .chain()
                .in_set(WorldOverlaySystems::Sync),
        );
    }
}

#[derive(Component, Default, Clone)]
struct TargetRing;

fn ring() -> impl Scene {
    let halo = glow(RING_COLOR);
    bsn! {
        TargetRing
        @hung_root(RING_SIZE.x, RING_Z)
        Node {
            height: px(RING_SIZE.y),
            border: px(2),
            border_radius: BorderRadius::MAX_ELLIPTICAL,
        }
        BorderColor::all(RING_COLOR)
        halo
    }
}

/// Opacity of the ring `secs` into the session: full, down to half and back.
fn pulse_alpha(secs: f32) -> f32 {
    0.75 + 0.25 * (secs * TAU / PULSE_SECS).cos()
}

/// Keeps the ring on the locked unit and drops it when the lock moves or clears.
fn sync_ring(
    mut commands: Commands,
    locked: LockedUnit,
    rings: Query<(Entity, &WorldAnchor), With<TargetRing>>,
) {
    let target = locked.entity();
    let mut ringed = false;
    for (entity, anchor) in &rings {
        if Some(anchor.target) == target {
            ringed = true;
        } else {
            commands.entity(entity).despawn();
        }
    }
    let Some(target) = target.filter(|_| !ringed) else {
        return;
    };
    commands.spawn_scene(ring()).insert(WorldAnchor {
        target,
        width: RING_SIZE.x,
        drop: -RING_SIZE.y / 2.0,
    });
}

fn pulse_ring(time: Res<Time>, mut rings: Query<&mut BorderColor, With<TargetRing>>) {
    let color = RING_COLOR.with_alpha(pulse_alpha(time.elapsed_secs()));
    for mut border in &mut rings {
        *border = BorderColor::all(color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use lifthrasir_client::domain::entities::EntityRegistry;
    use lifthrasir_client::domain::input::LockedTarget;

    fn ring_targets(app: &mut App) -> Vec<Entity> {
        let world = app.world_mut();
        world
            .query_filtered::<&WorldAnchor, With<TargetRing>>()
            .iter(world)
            .map(|anchor| anchor.target)
            .collect()
    }

    #[test]
    fn the_ring_follows_the_lock_and_leaves_with_it() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_resource::<LockedTarget>()
            .init_resource::<EntityRegistry>()
            .add_systems(Update, sync_ring);
        let [first, second] = [7, 8].map(|gid| {
            let unit = app.world_mut().spawn_empty().id();
            app.world_mut()
                .resource_mut::<EntityRegistry>()
                .register_entity(gid, unit);
            unit
        });

        app.update();
        assert!(ring_targets(&mut app).is_empty());

        app.world_mut().resource_mut::<LockedTarget>().gid = Some(7);
        app.update();
        app.update();
        assert_eq!(ring_targets(&mut app), [first]);

        app.world_mut().resource_mut::<LockedTarget>().gid = Some(8);
        app.update();
        app.update();
        assert_eq!(ring_targets(&mut app), [second]);

        app.world_mut().resource_mut::<LockedTarget>().gid = None;
        app.update();
        assert!(ring_targets(&mut app).is_empty());
    }

    #[test]
    fn the_pulse_stays_between_half_and_full() {
        assert_eq!(pulse_alpha(0.0), 1.0);
        assert!((pulse_alpha(PULSE_SECS / 2.0) - 0.5).abs() < 1e-4);
        assert!((0..100).all(|step| (0.5..=1.0).contains(&pulse_alpha(step as f32 * 0.07))));
    }
}
