//! Server-authoritative displacement without walking interpolation.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use moonshine_behavior::prelude::*;
use net_contract::events::KnockedBack;

use super::{MovementState, MovementTarget, snapshot::SnapshotBuffer};
use crate::{
    core::state::GameState,
    domain::{
        entities::{
            character::states::AnimationState, pathfinding::WalkablePath, registry::EntityRegistry,
        },
        system_sets::MovementSystems,
    },
    utils::coordinates::spawn_coords_to_world_position,
};

/// Applies knockback after ordinary movement and before terrain alignment.
/// Clears buffered positions so interpolation cannot replay pre-knockback movement.
/// Only walking returns to idle; pending combat transitions remain untouched.
#[auto_add_system(
    plugin = super::plugin::MovementDomainPlugin,
    schedule = Update,
    config(
        after = MovementSystems::Confirm,
        after = MovementSystems::Interpolate,
        after = MovementSystems::Stop,
        before = MovementSystems::TerrainAlignment,
        run_if = in_state(GameState::InGame)
    )
)]
pub fn handle_knockback_system(
    mut events: MessageReader<KnockedBack>,
    mut commands: Commands,
    registry: Res<EntityRegistry>,
    mut transforms: Query<&mut Transform>,
    mut animations: Query<(&AnimationState, &mut Transition<AnimationState>)>,
) {
    for event in events.read() {
        let Some(entity) = registry.get_entity(event.unit_id) else {
            debug!("Knockback for unknown unit {}", event.unit_id);
            continue;
        };
        let (Ok(x), Ok(y)) = (u16::try_from(event.dst_x), u16::try_from(event.dst_y)) else {
            warn!(
                "Knockback destination outside cell range: ({}, {})",
                event.dst_x, event.dst_y
            );
            continue;
        };
        let Ok(mut transform) = transforms.get_mut(entity) else {
            continue;
        };

        let destination = spawn_coords_to_world_position(x, y);
        transform.translation.x = destination.x;
        transform.translation.z = destination.z;
        commands
            .entity(entity)
            .remove::<(MovementTarget, WalkablePath, SnapshotBuffer)>()
            .insert(MovementState::Idle);

        if let Ok((state, mut transition)) = animations.get_mut(entity)
            && (matches!(*transition, Transition::Next(AnimationState::Walking))
                || (transition.is_none() && *state == AnimationState::Walking))
        {
            *transition = Transition::Next(AnimationState::Idle);
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::{prelude::*, state::app::StatesPlugin};
    use moonshine_behavior::prelude::*;
    use net_contract::{NetContractPlugin, events::KnockedBack};

    use crate::{
        core::state::GameState,
        domain::entities::{
            character::{
                components::visual::{CharacterDirection, Direction},
                events::StatusParameterChanged,
                states::AnimationState,
            },
            movement::{MovementPlugin, MovementSpeed, MovementState, MovementTarget},
            pathfinding::WalkablePath,
            registry::EntityRegistry,
        },
        utils::coordinates::spawn_coords_to_world_position,
    };

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            NetContractPlugin,
            BehaviorPlugin::<AnimationState>::default(),
            MovementPlugin,
        ))
        .insert_state(GameState::InGame)
        .init_resource::<EntityRegistry>()
        .add_message::<StatusParameterChanged>()
        .add_systems(PostUpdate, transition::<AnimationState>);
        app
    }

    #[test]
    fn knockback_discards_remote_history_then_accepts_new_snapshots() {
        use bevy::time::TimeUpdateStrategy;
        use net_contract::events::{SnapshotReceived, ZoneSnapshotEntity};
        use std::time::Duration;

        let mut app = app();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_xyz(50.0, 7.0, -50.0),
                CharacterDirection {
                    facing: Direction::East,
                },
                MovementState::Idle,
                AnimationState::Hit,
            ))
            .id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(42, entity);
        app.world_mut().write_message(SnapshotReceived {
            server_tick: 100,
            entities: vec![ZoneSnapshotEntity {
                id: 42,
                x: 10,
                y: 10,
                dir: 6,
                move_state: 1,
                hp_pct: 100,
            }],
        });
        app.world_mut().write_message(KnockedBack {
            unit_id: 42,
            dst_x: 8,
            dst_y: 12,
        });

        app.update();
        app.update();

        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().translation,
            Vec3::new(40.0, 7.0, -60.0)
        );
        assert_eq!(
            *app.world().get::<MovementState>(entity).unwrap(),
            MovementState::Idle
        );
        assert_eq!(
            *app.world().get::<AnimationState>(entity).unwrap(),
            AnimationState::Hit
        );
        assert_eq!(
            app.world()
                .get::<CharacterDirection>(entity)
                .unwrap()
                .facing,
            Direction::East
        );

        app.world_mut().write_message(SnapshotReceived {
            server_tick: 500,
            entities: vec![ZoneSnapshotEntity {
                id: 42,
                x: 9,
                y: 12,
                dir: 6,
                move_state: 0,
                hp_pct: 100,
            }],
        });
        app.update();
        app.update();

        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().translation,
            Vec3::new(45.0, 7.0, -60.0)
        );
    }

    #[test]
    fn knockback_cancels_a_same_frame_walk_confirmation() {
        use net_contract::events::SelfMoved;

        let mut app = app();
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_xyz(50.0, 7.0, -50.0),
                CharacterDirection {
                    facing: Direction::East,
                },
                MovementState::Idle,
                MovementSpeed::default(),
                AnimationState::Idle,
            ))
            .id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .set_local_player(entity, 42);
        app.world_mut().write_message(SelfMoved {
            src_x: 10,
            src_y: 10,
            dst_x: 20,
            dst_y: 10,
            start_time: 100,
        });
        app.world_mut().write_message(KnockedBack {
            unit_id: 42,
            dst_x: 8,
            dst_y: 12,
        });

        app.update();
        app.update();

        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().translation,
            Vec3::new(40.0, 7.0, -60.0)
        );
        assert_eq!(
            *app.world().get::<AnimationState>(entity).unwrap(),
            AnimationState::Idle
        );
    }

    #[test]
    fn knockback_preserves_a_pending_attack_and_cleans_idle_movement() {
        let mut app = app();
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_xyz(50.0, 7.0, -50.0),
                CharacterDirection {
                    facing: Direction::West,
                },
                MovementState::Idle,
                MovementTarget::new(
                    10,
                    10,
                    20,
                    10,
                    spawn_coords_to_world_position(10, 10),
                    spawn_coords_to_world_position(20, 10),
                    0,
                ),
                WalkablePath::new(vec![(10, 10), (20, 10)], (20, 10)),
                AnimationState::Walking,
            ))
            .id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(42, entity);
        app.update();
        app.world_mut()
            .query::<BehaviorMut<AnimationState>>()
            .get_mut(app.world_mut(), entity)
            .unwrap()
            .start(AnimationState::Attacking);
        app.world_mut().write_message(KnockedBack {
            unit_id: 42,
            dst_x: 8,
            dst_y: 12,
        });

        app.update();

        assert_eq!(
            *app.world().get::<AnimationState>(entity).unwrap(),
            AnimationState::Attacking
        );
        assert_eq!(
            app.world()
                .get::<CharacterDirection>(entity)
                .unwrap()
                .facing,
            Direction::West
        );
        assert!(app.world().get::<MovementTarget>(entity).is_none());
        assert!(app.world().get::<WalkablePath>(entity).is_none());
        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().translation,
            Vec3::new(40.0, 7.0, -60.0)
        );
    }

    #[test]
    fn knockback_skips_unknown_and_despawned_units_without_losing_valid_events() {
        let mut app = app();
        let stale = app.world_mut().spawn(Transform::default()).id();
        let live = app.world_mut().spawn(Transform::default()).id();
        {
            let mut registry = app.world_mut().resource_mut::<EntityRegistry>();
            registry.register_entity(41, stale);
            registry.register_entity(42, live);
        }
        app.world_mut().despawn(stale);
        for unit_id in [40, 41, 42] {
            app.world_mut().write_message(KnockedBack {
                unit_id,
                dst_x: 8,
                dst_y: 12,
            });
        }

        app.update();

        assert_eq!(
            app.world().get::<Transform>(live).unwrap().translation,
            Vec3::new(40.0, 0.0, -60.0)
        );
    }

    #[test]
    fn knockback_rejects_coordinates_that_would_wrap_in_cell_space() {
        let mut app = app();
        let entity = app
            .world_mut()
            .spawn(Transform::from_xyz(50.0, 7.0, -50.0))
            .id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(42, entity);
        for (dst_x, dst_y) in [(65_536, 12), (8, 65_536)] {
            app.world_mut().write_message(KnockedBack {
                unit_id: 42,
                dst_x,
                dst_y,
            });
        }

        app.update();

        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().translation,
            Vec3::new(50.0, 7.0, -50.0)
        );
    }

    #[test]
    fn knockback_stops_local_walking_at_the_server_destination() {
        let mut app = app();
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_xyz(50.0, 7.0, -50.0),
                CharacterDirection {
                    facing: Direction::East,
                },
                MovementState::Moving,
                MovementSpeed::default(),
                MovementTarget::new(
                    10,
                    10,
                    20,
                    10,
                    spawn_coords_to_world_position(10, 10),
                    spawn_coords_to_world_position(20, 10),
                    0,
                ),
                WalkablePath::new(vec![(10, 10), (20, 10)], (20, 10)),
                AnimationState::Walking,
            ))
            .id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .set_local_player(entity, 42);
        app.world_mut().write_message(KnockedBack {
            unit_id: 42,
            dst_x: 8,
            dst_y: 12,
        });

        app.update();
        app.update();

        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().translation,
            Vec3::new(40.0, 7.0, -60.0)
        );
        assert!(app.world().get::<MovementTarget>(entity).is_none());
        assert!(app.world().get::<WalkablePath>(entity).is_none());
        assert_eq!(
            *app.world().get::<MovementState>(entity).unwrap(),
            MovementState::Idle
        );
        assert_eq!(
            *app.world().get::<AnimationState>(entity).unwrap(),
            AnimationState::Idle
        );
    }
}
