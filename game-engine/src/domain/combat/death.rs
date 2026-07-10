use super::components::DeadEntity;
use crate::domain::{
    entities::{
        character::components::status::CharacterStatus, character::states::AnimationState,
        markers::LocalPlayer,
    },
    system_sets::CombatSystems,
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use moonshine_behavior::prelude::*;

/// Detects the local player's own death.
///
/// A player's own death is signalled only by the HP param reaching 0 — the
/// server sends no vanish/`UnitLeft` for self (that goes to other nearby
/// players). We therefore watch the *applied* HP on the `LocalPlayer`
/// (`CharacterStatus.hp`, written by `update_character_status_system` after it
/// drains `PendingStatusParams`) rather than the raw `ParamChanged` message, so
/// a buffered/initial login param is never mis-read as a death.
///
/// `Without<DeadEntity>` makes this idempotent: once the corpse marker is on,
/// the entity no longer matches and re-death is a no-op.
type LocalDeathQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static CharacterStatus,
        BehaviorMut<AnimationState>,
    ),
    (
        With<LocalPlayer>,
        Without<DeadEntity>,
        Changed<CharacterStatus>,
    ),
>;

#[auto_add_system(
    plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update,
    config(in_set = CombatSystems::HandleDeath)
)]
pub fn detect_local_death(mut commands: Commands, mut player: LocalDeathQuery) {
    let Ok((entity, status, mut behavior)) = player.single_mut() else {
        return;
    };

    if status.hp != 0 {
        return;
    }

    commands.entity(entity).insert(DeadEntity);
    behavior.reset();
    behavior.start(AnimationState::Dead);
}

/// Recovers the local player once its HP rises above 0 again.
///
/// aesir does not emit an inbound respawn packet: on `Respawn` it restores the
/// HP/SP params to max via `send_params` and warps (`Body::Respawn` is
/// client→server only and never drained). Both respawn paths — save-point warp
/// and resurrect-in-place — therefore surface as the applied HP going positive.
/// Recovering off that applied HP is the mirror of `detect_local_death` and
/// covers both paths without depending on any respawn event.
///
/// `With<DeadEntity>` + `hp > 0` is the exact complement of the death detector's
/// `Without<DeadEntity>` + `hp == 0`, so there is no window where a still-dead
/// entity is re-killed or prematurely revived by an unrelated status change.
///
/// `Dead` is a terminal `AnimationState` (no transitions out), so recovery uses
/// `reset()` — which resumes the initial behavior and bypasses `filter_next` —
/// rather than `start(Idle)`, which the terminal filter would reject. Entities
/// are spawned with `AnimationState::Idle` as their initial behavior.
type LocalReviveQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static CharacterStatus,
        BehaviorMut<AnimationState>,
    ),
    (
        With<LocalPlayer>,
        With<DeadEntity>,
        Changed<CharacterStatus>,
    ),
>;

#[auto_add_system(
    plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update,
    config(in_set = CombatSystems::HandleDeath)
)]
pub fn recover_local_from_hp(mut commands: Commands, mut player: LocalReviveQuery) {
    let Ok((entity, status, mut behavior)) = player.single_mut() else {
        return;
    };

    if status.hp == 0 {
        return;
    }

    commands.entity(entity).remove::<DeadEntity>();
    behavior.reset();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn death_app() -> App {
        let mut app = App::new();
        app.add_plugins(BehaviorPlugin::<AnimationState>::default())
            .add_systems(
                Update,
                (
                    detect_local_death,
                    recover_local_from_hp,
                    transition::<AnimationState>,
                )
                    .chain(),
            );
        app
    }

    fn status(hp: u32) -> CharacterStatus {
        CharacterStatus {
            hp,
            ..Default::default()
        }
    }

    fn state(app: &App, entity: Entity) -> AnimationState {
        *app.world().get::<AnimationState>(entity).unwrap()
    }

    #[test]
    fn local_player_hp_reaching_zero_kills_the_entity() {
        let mut app = death_app();
        let player = app
            .world_mut()
            .spawn((LocalPlayer, AnimationState::Idle, status(0)))
            .id();

        app.update();

        assert!(app.world().get::<DeadEntity>(player).is_some());
        assert_eq!(state(&app, player), AnimationState::Dead);
    }

    #[test]
    fn hp_zero_without_local_player_does_nothing() {
        let mut app = death_app();
        let unit = app
            .world_mut()
            .spawn((AnimationState::Idle, status(0)))
            .id();

        app.update();

        assert!(app.world().get::<DeadEntity>(unit).is_none());
        assert_eq!(state(&app, unit), AnimationState::Idle);
    }

    #[test]
    fn hp_rising_above_zero_clears_death_and_resets_to_idle() {
        let mut app = death_app();
        let player = app
            .world_mut()
            .spawn((LocalPlayer, AnimationState::Idle, status(0)))
            .id();

        app.update();
        assert!(app.world().get::<DeadEntity>(player).is_some());
        assert_eq!(state(&app, player), AnimationState::Dead);

        app.world_mut()
            .get_mut::<CharacterStatus>(player)
            .unwrap()
            .hp = 5000;
        app.update();

        assert!(app.world().get::<DeadEntity>(player).is_none());
        assert_eq!(state(&app, player), AnimationState::Idle);
    }

    #[test]
    fn re_death_when_already_dead_is_a_no_op() {
        let mut app = death_app();
        let player = app
            .world_mut()
            .spawn((LocalPlayer, AnimationState::Idle, status(0)))
            .id();

        app.update();
        assert!(app.world().get::<DeadEntity>(player).is_some());

        // Touch the status again (marks it Changed) while still dead.
        app.world_mut()
            .get_mut::<CharacterStatus>(player)
            .unwrap()
            .hp = 0;
        app.update();

        assert!(app.world().get::<DeadEntity>(player).is_some());
        assert_eq!(state(&app, player), AnimationState::Dead);
    }

    #[test]
    fn dead_entity_with_hp_still_zero_does_not_recover_on_other_status_change() {
        let mut app = death_app();
        let player = app
            .world_mut()
            .spawn((LocalPlayer, AnimationState::Idle, status(0)))
            .id();

        app.update();
        assert!(app.world().get::<DeadEntity>(player).is_some());

        // An unrelated field changes while HP stays 0: must not revive.
        app.world_mut()
            .get_mut::<CharacterStatus>(player)
            .unwrap()
            .sp = 42;
        app.update();

        assert!(app.world().get::<DeadEntity>(player).is_some());
        assert_eq!(state(&app, player), AnimationState::Dead);
    }
}
