//! Plays the local player's pickup motion on a successful `PickupResult`.
//!
//! Kept separate from [`super::pickup`] (which handles the request/result chat
//! feedback) so the animation concern doesn't overload that system.

use bevy::prelude::*;
use moonshine_behavior::prelude::*;
use net_contract::events::{PickupOutcome, PickupResult};

use crate::domain::entities::character::states::AnimationState;
use crate::domain::entities::markers::LocalPlayer;

const PICKUP_ANIM_SECS: f32 = 0.5;

/// One-shot timer for the local player's pickup animation; removed (and the
/// animation returned to `Idle`) when it finishes. Mirrors `combat::HitStun`.
#[derive(Component)]
pub struct PickupAnimTimer(pub Timer);

impl Default for PickupAnimTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(PICKUP_ANIM_SECS, TimerMode::Once))
    }
}

/// Drives the local player's `PickingUp` animation on a successful pickup.
/// Error outcomes are ignored — only the chat line (`handle_pickup_result`)
/// reacts to those.
pub fn play_pickup_animation(
    mut commands: Commands,
    mut results: MessageReader<PickupResult>,
    local_player: Query<Entity, With<LocalPlayer>>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
) {
    let succeeded = results
        .read()
        .any(|result| result.outcome == PickupOutcome::Ok);
    if !succeeded {
        return;
    }

    let Ok(player) = local_player.single() else {
        return;
    };

    let Ok(mut behavior) = behaviors.get_mut(player) else {
        return;
    };

    behavior.start(AnimationState::PickingUp);
    commands.entity(player).insert(PickupAnimTimer::default());
}

/// Returns the local player to `Idle` once the pickup animation has played out.
pub fn tick_pickup_anim(
    mut commands: Commands,
    time: Res<Time>,
    mut timers: Query<(Entity, &mut PickupAnimTimer)>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
) {
    for (entity, mut timer) in timers.iter_mut() {
        timer.0.tick(time.delta());

        if timer.0.just_finished() {
            commands.entity(entity).remove::<PickupAnimTimer>();

            if let Ok(mut behavior) = behaviors.get_mut(entity) {
                behavior.start(AnimationState::Idle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin);
        app.add_plugins(BehaviorPlugin::<AnimationState>::default());
        app.add_message::<PickupResult>();
        app
    }

    #[test]
    fn ok_outcome_starts_picking_up_and_inserts_timer() {
        let mut app = test_app();
        app.add_systems(
            Update,
            (play_pickup_animation, transition::<AnimationState>).chain(),
        );

        let player = app
            .world_mut()
            .spawn((LocalPlayer, AnimationState::Idle))
            .id();

        app.world_mut()
            .resource_mut::<Messages<PickupResult>>()
            .write(PickupResult {
                ground_id: 1,
                outcome: PickupOutcome::Ok,
            });
        app.update();

        let world = app.world();
        assert!(world.get::<PickupAnimTimer>(player).is_some());
        assert_eq!(
            *world.get::<AnimationState>(player).unwrap(),
            AnimationState::PickingUp
        );
    }

    #[test]
    fn error_outcome_triggers_no_animation() {
        let mut app = test_app();
        app.add_systems(
            Update,
            (play_pickup_animation, transition::<AnimationState>).chain(),
        );

        let player = app
            .world_mut()
            .spawn((LocalPlayer, AnimationState::Idle))
            .id();

        app.world_mut()
            .resource_mut::<Messages<PickupResult>>()
            .write(PickupResult {
                ground_id: 1,
                outcome: PickupOutcome::TooFar,
            });
        app.update();

        let world = app.world();
        assert!(world.get::<PickupAnimTimer>(player).is_none());
        assert_eq!(
            *world.get::<AnimationState>(player).unwrap(),
            AnimationState::Idle
        );
    }

    #[test]
    fn no_local_player_is_a_noop() {
        let mut app = test_app();
        app.add_systems(
            Update,
            (play_pickup_animation, transition::<AnimationState>).chain(),
        );

        app.world_mut()
            .resource_mut::<Messages<PickupResult>>()
            .write(PickupResult {
                ground_id: 1,
                outcome: PickupOutcome::Ok,
            });

        app.update();
    }

    fn tick_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin);
        app.add_plugins(BehaviorPlugin::<AnimationState>::default());
        app.add_systems(
            Update,
            (tick_pickup_anim, transition::<AnimationState>).chain(),
        );
        app
    }

    #[test]
    fn finished_timer_removes_itself_and_returns_to_idle() {
        let mut app = tick_app();

        let entity = app
            .world_mut()
            .spawn((
                AnimationState::PickingUp,
                PickupAnimTimer(Timer::from_seconds(0.0, TimerMode::Once)),
            ))
            .id();

        app.update();

        let world = app.world();
        assert!(world.get::<PickupAnimTimer>(entity).is_none());
        assert_eq!(
            *world.get::<AnimationState>(entity).unwrap(),
            AnimationState::Idle
        );
    }

    #[test]
    fn unfinished_timer_keeps_animating() {
        let mut app = tick_app();

        let entity = app
            .world_mut()
            .spawn((AnimationState::PickingUp, PickupAnimTimer::default()))
            .id();

        app.update();

        let world = app.world();
        assert!(world.get::<PickupAnimTimer>(entity).is_some());
        assert_eq!(
            *world.get::<AnimationState>(entity).unwrap(),
            AnimationState::PickingUp
        );
    }
}
