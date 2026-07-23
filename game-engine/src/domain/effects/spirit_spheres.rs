use std::collections::HashMap;
use std::f32::consts::TAU;

use bevy::prelude::*;
use net_contract::events::{SpiritSphereChanged, UnitEntered};

use crate::domain::entities::registry::EntityRegistry;

/// Orbit radius, in world units, of each spirit sphere — wide enough that the
/// ring clears the body sprite instead of clipping through it.
const SPHERE_ORBIT_RADIUS: f32 = 11.0;
/// Full revolution period of the sphere ring.
const SPHERE_ORBIT_PERIOD_SECS: f32 = 3.0;
/// Vertical offset lifting the ring to the unit's upper body; world up is `-Y`.
const SPHERE_ORBIT_LIFT: f32 = -10.0;
/// Amplitude of the per-sphere vertical bob.
const SPHERE_BOB_AMPLITUDE: f32 = 0.8;
/// Period of the per-sphere vertical bob.
const SPHERE_BOB_PERIOD_SECS: f32 = 1.6;
/// Phase offset between neighbouring spheres' bobs so the ring undulates
/// instead of moving as one rigid plate.
const SPHERE_BOB_PHASE_STEP: f32 = 1.7;

/// One spirit-sphere anchor orbiting `unit`. A TOP-LEVEL entity following the
/// unit from the outside (the same pattern as [`super::SightOrbit`] — effect
/// visuals do not render reliably as unit children). The domain spawns only
/// this bare anchor, moved every frame by [`orbit_spirit_spheres`]; the
/// presentation layer dresses it with the ki-ball billboard
/// (`dress_spirit_sphere_orbits` in `presentation/rendering/effects/skill_fx.rs`).
#[derive(Component, Debug, Clone, Copy)]
pub struct SpiritSphereOrbit {
    pub unit: Entity,
    /// This sphere's slot within the ring, `0..count`.
    pub index: u32,
    /// Ring size when spawned; fixes each sphere's angular offset.
    pub count: u32,
}

/// Latest sphere count for units whose entity has not registered yet, keyed by
/// unit_id. Same shape and rationale as [`super::PendingBodyStates`].
#[derive(Resource, Default)]
pub struct PendingSpiritSpheres(HashMap<u32, u32>);

/// Reconciles a unit's orbiting sphere ring with the authoritative count:
/// consumes live [`SpiritSphereChanged`] updates and the spawn-time
/// [`UnitEntered`] count for units that enter view with spheres already up,
/// mirroring [`super::option_visuals`]. A count change rebuilds the whole ring
/// so the spheres stay evenly spaced.
pub fn spirit_sphere_visuals(
    mut changes: MessageReader<SpiritSphereChanged>,
    mut entered: MessageReader<UnitEntered>,
    registry: Res<EntityRegistry>,
    mut pending: ResMut<PendingSpiritSpheres>,
    mut commands: Commands,
    transforms: Query<&Transform>,
    orbits: Query<(Entity, &SpiritSphereOrbit)>,
) {
    for event in changes.read() {
        let Some(entity) = registry.get_entity(event.unit_id) else {
            pending.0.insert(event.unit_id, event.count);
            continue;
        };
        apply_sphere_count(&mut commands, entity, event.count, &transforms, &orbits);
    }

    for event in entered.read() {
        if event.spirit_sphere_count == 0 {
            continue;
        }
        let Some(entity) = registry.get_entity(event.gid) else {
            continue;
        };
        apply_sphere_count(
            &mut commands,
            entity,
            event.spirit_sphere_count,
            &transforms,
            &orbits,
        );
    }

    pending.0.retain(|&unit_id, &mut count| {
        let Some(entity) = registry.get_entity(unit_id) else {
            return true;
        };
        apply_sphere_count(&mut commands, entity, count, &transforms, &orbits);
        false
    });
}

fn apply_sphere_count(
    commands: &mut Commands,
    entity: Entity,
    count: u32,
    transforms: &Query<&Transform>,
    orbits: &Query<(Entity, &SpiritSphereOrbit)>,
) {
    let existing: Vec<Entity> = orbits
        .iter()
        .filter(|(_, orbit)| orbit.unit == entity)
        .map(|(orbit_entity, _)| orbit_entity)
        .collect();
    if existing.len() == count as usize {
        return;
    }

    for orbit_entity in existing {
        commands.entity(orbit_entity).despawn();
    }

    // Start at the unit so the first frame never flashes at the origin.
    let start = transforms
        .get(entity)
        .map(|t| t.translation)
        .unwrap_or_default();
    for index in 0..count {
        commands.spawn((
            SpiritSphereOrbit {
                unit: entity,
                index,
                count,
            },
            Transform::from_translation(start),
            Visibility::default(),
        ));
    }
}

/// Moves every [`SpiritSphereOrbit`] anchor along the shared ring around its
/// unit's current position, each sphere offset by its slot angle and bobbing
/// gently out of phase with its neighbours. An orbit whose unit is gone
/// (despawned, map change) is despawned here, taking the dressed ball with it.
pub fn orbit_spirit_spheres(
    time: Res<Time>,
    mut commands: Commands,
    units: Query<&GlobalTransform>,
    mut orbits: Query<(Entity, &SpiritSphereOrbit, &mut Transform)>,
) {
    let elapsed = time.elapsed_secs();
    let base_angle = elapsed * (TAU / SPHERE_ORBIT_PERIOD_SECS);
    for (orbit_entity, orbit, mut transform) in &mut orbits {
        let Ok(unit) = units.get(orbit.unit) else {
            commands.entity(orbit_entity).despawn();
            continue;
        };
        let slot = orbit.index as f32 * TAU / orbit.count.max(1) as f32;
        let (sin, cos) = (base_angle + slot).sin_cos();
        let bob = (elapsed * (TAU / SPHERE_BOB_PERIOD_SECS)
            + orbit.index as f32 * SPHERE_BOB_PHASE_STEP)
            .sin()
            * SPHERE_BOB_AMPLITUDE;
        transform.translation = unit.translation()
            + Vec3::new(
                cos * SPHERE_ORBIT_RADIUS,
                SPHERE_ORBIT_LIFT + bob,
                sin * SPHERE_ORBIT_RADIUS,
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<SpiritSphereChanged>()
            .add_message::<UnitEntered>()
            .init_resource::<EntityRegistry>()
            .init_resource::<PendingSpiritSpheres>()
            .add_systems(Update, spirit_sphere_visuals);
        app
    }

    fn register_unit(app: &mut App, unit_id: u32) -> Entity {
        let entity = app
            .world_mut()
            .spawn(Transform::from_xyz(10.0, 0.0, 20.0))
            .id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(unit_id, entity);
        entity
    }

    fn send_count(app: &mut App, unit_id: u32, count: u32) {
        app.world_mut()
            .resource_mut::<Messages<SpiritSphereChanged>>()
            .write(SpiritSphereChanged { unit_id, count });
    }

    fn orbit_count(app: &mut App, unit: Entity) -> usize {
        app.world_mut()
            .query::<&SpiritSphereOrbit>()
            .iter(app.world())
            .filter(|orbit| orbit.unit == unit)
            .count()
    }

    #[test]
    fn sphere_change_spawns_one_anchor_per_ball() {
        let mut app = app();
        let unit = register_unit(&mut app, 150001);

        send_count(&mut app, 150001, 3);
        app.update();

        assert_eq!(orbit_count(&mut app, unit), 3);
    }

    #[test]
    fn count_change_rebuilds_the_ring() {
        let mut app = app();
        let unit = register_unit(&mut app, 150001);

        send_count(&mut app, 150001, 5);
        app.update();
        send_count(&mut app, 150001, 2);
        app.update();
        // Despawn commands and respawns both applied; spacing comes from the
        // fresh anchors' count field.
        app.update();

        assert_eq!(orbit_count(&mut app, unit), 2);
        let counts: Vec<u32> = app
            .world_mut()
            .query::<&SpiritSphereOrbit>()
            .iter(app.world())
            .map(|orbit| orbit.count)
            .collect();
        assert!(counts.iter().all(|&count| count == 2));
    }

    #[test]
    fn zero_count_clears_all_spheres() {
        let mut app = app();
        let unit = register_unit(&mut app, 150001);

        send_count(&mut app, 150001, 4);
        app.update();
        send_count(&mut app, 150001, 0);
        app.update();

        assert_eq!(orbit_count(&mut app, unit), 0);
    }

    #[test]
    fn unresolved_unit_buffers_until_registered() {
        let mut app = app();

        send_count(&mut app, 150001, 3);
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&SpiritSphereOrbit>()
                .iter(app.world())
                .count(),
            0
        );

        let unit = register_unit(&mut app, 150001);
        app.update();

        assert_eq!(orbit_count(&mut app, unit), 3);
    }

    #[test]
    fn entered_with_spheres_dresses_on_spawn() {
        let mut app = app();
        let unit = register_unit(&mut app, 150001);

        app.world_mut()
            .resource_mut::<Messages<UnitEntered>>()
            .write(UnitEntered {
                gid: 150001,
                spirit_sphere_count: 5,
                ..sample_entered()
            });
        app.update();

        assert_eq!(orbit_count(&mut app, unit), 5);
    }

    fn sample_entered() -> UnitEntered {
        UnitEntered {
            gid: 0,
            aid: 0,
            object_type: 0,
            job: 0,
            x: 0,
            y: 0,
            dir: 0,
            speed: 150,
            hp: 100,
            max_hp: 100,
            clevel: 1,
            body_state: 0,
            health_state: 0,
            effect_state: 0,
            virtue: 0,
            spirit_sphere_count: 0,
            head: 0,
            weapon: 0,
            shield: 0,
            accessory: 0,
            accessory2: 0,
            accessory3: 0,
            head_palette: 0,
            body_palette: 0,
            head_dir: 0,
            robe: 0,
            guild_id: 0,
            guild_name: String::new(),
            emblem_id: 0,
            sex: 0,
            is_boss: false,
            name: String::new(),
            moving: false,
            dst_x: 0,
            dst_y: 0,
            move_start_time: 0,
        }
    }
}
