//! Casting pose: on `SkillCastStarted` with a real cast time the caster turns
//! toward its target and holds the ACT casting action until the cast resolves
//! (`CastTimer` expiry) or is interrupted (`CastCancelled`). The pose only
//! reverts to Idle if the unit is still Casting, so a skill's own attack motion
//! or a flinch that landed meanwhile is left alone.

use super::components::DeadEntity;
use crate::domain::audio::events::PlaySkillSfx;
use crate::domain::{
    entities::{
        character::{components::visual::CharacterDirection, states::AnimationState},
        registry::EntityRegistry,
    },
    system_sets::CombatSystems,
};
use crate::utils::coordinates::Direction;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use moonshine_behavior::prelude::*;
use net_contract::events::{CastCancelled, SkillCastStarted};

/// Holds the casting pose for the cast's duration.
#[derive(Component, Debug, Clone)]
pub struct CastTimer {
    pub timer: Timer,
}

/// The reference client's generic cast-start chime, played for every timed cast.
const CAST_START_SFX: &str = "effect/ef_beginspell.wav";

#[auto_add_system(
    plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update,
    config(in_set = CombatSystems::ProcessActions)
)]
pub fn start_cast_pose(
    mut commands: Commands,
    mut events: MessageReader<SkillCastStarted>,
    registry: Res<EntityRegistry>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
    transforms: Query<&Transform>,
    mut sfx: MessageWriter<PlaySkillSfx>,
) {
    for event in events.read() {
        if event.cast_time == 0 {
            continue;
        }
        let Some(caster) = registry.get_entity(event.src_id) else {
            continue;
        };

        sfx.write(PlaySkillSfx {
            emitter: caster,
            sound: CAST_START_SFX.to_string(),
        });

        // Face the target while casting. Self- and ground-targeted casts keep
        // the current facing (a self-cast yields a zero vector).
        if let Some(target) = registry.get_entity(event.target_id) {
            if let (Ok(src_t), Ok(target_t)) = (transforms.get(caster), transforms.get(target)) {
                let dx = target_t.translation.x - src_t.translation.x;
                let dz = target_t.translation.z - src_t.translation.z;
                if dx != 0.0 || dz != 0.0 {
                    commands.entity(caster).insert(CharacterDirection {
                        facing: Direction::from_movement_vector(dx, dz),
                    });
                }
            }
        }

        commands.entity(caster).insert(CastTimer {
            timer: Timer::from_seconds(event.cast_time as f32 / 1000.0, TimerMode::Once),
        });
        if let Ok(mut behavior) = behaviors.get_mut(caster) {
            behavior.start(AnimationState::Casting);
        }
    }
}

#[auto_add_system(
    plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update,
    config(in_set = CombatSystems::UpdateTimers)
)]
pub fn update_cast_timers(
    mut commands: Commands,
    time: Res<Time>,
    mut timers: Query<(Entity, &mut CastTimer), Without<DeadEntity>>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
) {
    for (entity, mut cast) in timers.iter_mut() {
        cast.timer.tick(time.delta());
        if !cast.timer.just_finished() {
            continue;
        }
        commands.entity(entity).remove::<CastTimer>();

        if let Ok(mut behavior) = behaviors.get_mut(entity) {
            if *behavior.current() == AnimationState::Casting {
                behavior.start(AnimationState::Idle);
            }
        }
    }
}

#[auto_add_system(
    plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update,
    config(in_set = CombatSystems::ProcessActions)
)]
pub fn cancel_cast_pose(
    mut commands: Commands,
    mut events: MessageReader<CastCancelled>,
    registry: Res<EntityRegistry>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
) {
    for event in events.read() {
        let Some(entity) = registry.get_entity(event.gid) else {
            continue;
        };
        commands.entity(entity).remove::<CastTimer>();

        if let Ok(mut behavior) = behaviors.get_mut(entity) {
            if *behavior.current() == AnimationState::Casting {
                behavior.start(AnimationState::Idle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cast_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin)
            .add_plugins(BehaviorPlugin::<AnimationState>::default())
            .init_resource::<EntityRegistry>()
            .add_message::<SkillCastStarted>()
            .add_message::<CastCancelled>()
            .add_message::<PlaySkillSfx>()
            .add_systems(
                Update,
                (
                    start_cast_pose,
                    cancel_cast_pose,
                    update_cast_timers,
                    transition::<AnimationState>,
                )
                    .chain(),
            );
        app
    }

    fn spawn_caster(app: &mut App, gid: u32) -> Entity {
        let caster = app
            .world_mut()
            .spawn((Transform::default(), AnimationState::Idle))
            .id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(gid, caster);
        caster
    }

    fn cast_started(src_id: u32, cast_time: u32) -> SkillCastStarted {
        SkillCastStarted {
            src_id,
            target_id: 0,
            x: 0,
            y: 0,
            skill_id: 1,
            property: 0,
            cast_time,
        }
    }

    fn state(app: &App, entity: Entity) -> AnimationState {
        *app.world().get::<AnimationState>(entity).unwrap()
    }

    #[test]
    fn timed_cast_enters_casting_pose() {
        let mut app = cast_app();
        let caster = spawn_caster(&mut app, 7);

        app.world_mut().write_message(cast_started(7, 5000));
        app.update();

        assert_eq!(state(&app, caster), AnimationState::Casting);
        assert!(app.world().get::<CastTimer>(caster).is_some());

        let sfx: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<PlaySkillSfx>>()
            .drain()
            .collect();
        assert_eq!(sfx.len(), 1);
        assert_eq!(sfx[0].emitter, caster);
        assert_eq!(sfx[0].sound, CAST_START_SFX);
    }

    #[test]
    fn instant_cast_keeps_current_pose() {
        let mut app = cast_app();
        let caster = spawn_caster(&mut app, 7);

        app.world_mut().write_message(cast_started(7, 0));
        app.update();

        assert_eq!(state(&app, caster), AnimationState::Idle);
        assert!(app.world().get::<CastTimer>(caster).is_none());
        let sfx_count = app
            .world_mut()
            .resource_mut::<Messages<PlaySkillSfx>>()
            .drain()
            .count();
        assert_eq!(sfx_count, 0);
    }

    #[test]
    fn cast_timer_expiry_returns_to_idle() {
        let mut app = cast_app();
        let caster = spawn_caster(&mut app, 7);

        app.world_mut().write_message(cast_started(7, 5000));
        app.update();
        assert_eq!(state(&app, caster), AnimationState::Casting);

        let mut cast = app.world_mut().get_mut::<CastTimer>(caster).unwrap();
        let duration = cast.timer.duration();
        cast.timer.set_elapsed(duration);
        app.update();

        assert_eq!(state(&app, caster), AnimationState::Idle);
        assert!(app.world().get::<CastTimer>(caster).is_none());
    }

    #[test]
    fn cast_cancelled_returns_to_idle() {
        let mut app = cast_app();
        let caster = spawn_caster(&mut app, 7);

        app.world_mut().write_message(cast_started(7, 5000));
        app.update();
        assert_eq!(state(&app, caster), AnimationState::Casting);

        app.world_mut().write_message(CastCancelled { gid: 7 });
        app.update();

        assert_eq!(state(&app, caster), AnimationState::Idle);
        assert!(app.world().get::<CastTimer>(caster).is_none());
    }
}
