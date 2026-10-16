//! Translate client state into presentation requests without exposing gameplay to playback.

use bevy::prelude::*;
use lifthrasir_runtime::actors::{ActionType, ActorAnimationInput};
use std::time::Duration;

use crate::domain::combat::components::AttackTimer;
use crate::domain::entities::character::states::AnimationState;
use crate::domain::entities::movement::components::MovementSpeed;
use lifthrasir_runtime::actors::Gr2Actor;
use lifthrasir_runtime::actors::sprites::components::{MobSprite, PlayerSprite};
use lifthrasir_runtime::actors::sprites::layout::{ActionLayout, MobLayout, PlayerLayout};
use lifthrasir_runtime::effects::overlays::AnimationPaused;

#[doc(hidden)]
#[derive(Component, Default)]
pub struct ClientAnimationRequests {
    attack: Option<u64>,
    deferred: bool,
}

/// Install the client input phase used by both sprite and model playback.
pub(crate) fn register_actor_input_bridge(app: &mut App) {
    use moonshine_behavior::prelude::transition;
    use {
        crate::domain::system_sets::{
            ClientPresentationSystems, EntityLifecycleSystems, MovementSystems,
        },
        lifthrasir_runtime::rendering::SpriteRenderingSystems,
    };
    app.configure_sets(
        Update,
        SpriteRenderingSystems::HierarchySpawn.after(EntityLifecycleSystems::Spawning),
    );
    app.configure_sets(
        Update,
        ClientPresentationSystems
            .after(SpriteRenderingSystems::AnimationEvents)
            .after(EntityLifecycleSystems::Spawning)
            .after(MovementSystems::TerrainAlignment)
            .after(crate::domain::system_sets::CombatSystems::HandleDeath)
            .after(transition::<AnimationState>)
            .after(crate::domain::effects::body_state_visuals)
            .before(SpriteRenderingSystems::AnimationSync),
    )
    .add_systems(
        Update,
        sync_actor_animation_inputs.in_set(ClientPresentationSystems),
    );
}

const STANDARD_WALK_MS_PER_CELL: f32 = 150.0;

fn walk_speed_factor(action_type: ActionType, movement_speed: Option<&MovementSpeed>) -> f32 {
    if action_type != ActionType::Walk {
        return 1.0;
    }
    movement_speed.map_or(1.0, |speed| speed.ms_per_cell / STANDARD_WALK_MS_PER_CELL)
}

fn request_start(now: Duration, model: bool, timer: Option<&AttackTimer>) -> Duration {
    if model {
        timer.map_or(now, |timer| now.saturating_sub(timer.timer.elapsed()))
    } else {
        now
    }
}

type AnimationSources<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Ref<'static, AnimationState>,
        Option<&'static AttackTimer>,
        Option<&'static MovementSpeed>,
        Has<AnimationPaused>,
        Has<Gr2Actor>,
        Option<&'static PlayerSprite>,
        Option<&'static MobSprite>,
        Option<&'static mut ActorAnimationInput>,
        Option<&'static mut ClientAnimationRequests>,
    ),
>;

/// Capture new state/attack requests once; rate and pause updates never increment their ID.
pub fn sync_actor_animation_inputs(
    time: Res<Time>,
    mut sources: AnimationSources,
    mut commands: Commands,
) {
    let now = time.elapsed();
    for (entity, state, timer, movement, paused, model, player, mob, input, tracking) in
        &mut sources
    {
        let desired = ActionType::from(*state);
        let timer = timer.filter(|_| desired == ActionType::Attack);
        let attack = timer.map(|timer| timer.request_id);
        let duration = timer.map(|timer| timer.timer.duration());
        let factor = walk_speed_factor(
            desired,
            movement.filter(|speed| speed.ms_per_cell.is_finite() && speed.ms_per_cell > 0.0),
        );
        let looping = if mob.is_some() {
            MobLayout::is_looping(desired)
        } else {
            PlayerLayout::is_looping(desired)
        };
        let state_request = state.is_changed() && !looping;
        let previous_id = input.as_deref().map_or(0, |input| input.restart_id);
        if let (Some(mut input), Some(mut tracking)) = (input, tracking) {
            if paused {
                if state_request {
                    tracking.deferred = true;
                }
                if !input.paused {
                    input.paused = true;
                }
                continue;
            }
            let new_attack = attack.is_some() && attack != tracking.attack;
            let restart =
                desired != input.action || new_attack || state_request || tracking.deferred;
            let mut next = *input;
            next.paused = false;
            next.speed_factor = factor;
            next.duration = duration;
            if restart {
                next.action = desired;
                next.restart_id = next.restart_id.wrapping_add(1);
                next.started_at = request_start(now, model, timer);
            }
            input.set_if_neq(next);
            tracking.attack = attack;
            tracking.deferred = false;
            continue;
        }
        let sprite = player
            .map(|sprite| (sprite.action_type, sprite.start_time, sprite.speed_factor))
            .or_else(|| {
                mob.map(|sprite| (sprite.action_type, sprite.start_time, sprite.speed_factor))
            });
        let (action, started_at, speed_factor) =
            if paused && let Some((action, start, factor)) = sprite {
                (action, Duration::from_millis(u64::from(start)), factor)
            } else {
                let start = if !model && desired == ActionType::Idle {
                    match sprite {
                        Some((action, start, _)) if action == desired => {
                            Duration::from_millis(u64::from(start))
                        }
                        None => Duration::ZERO,
                        _ => now,
                    }
                } else {
                    request_start(now, model, timer)
                };
                (desired, start, factor)
            };
        commands.entity(entity).insert((
            ActorAnimationInput {
                action,
                restart_id: previous_id.wrapping_add(1),
                started_at,
                duration,
                speed_factor,
                paused,
            },
            ClientAnimationRequests {
                attack,
                deferred: paused && action != desired,
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_speed_keeps_natural_walk_rate() {
        let speed = MovementSpeed::from_server_speed(150);
        assert_eq!(walk_speed_factor(ActionType::Walk, Some(&speed)), 1.0);
    }

    #[test]
    fn slow_mob_stretches_walk_cycle() {
        let speed = MovementSpeed::from_server_speed(450);
        assert_eq!(walk_speed_factor(ActionType::Walk, Some(&speed)), 3.0);
    }

    #[test]
    fn non_walk_actions_play_at_natural_rate() {
        let speed = MovementSpeed::from_server_speed(450);
        assert_eq!(walk_speed_factor(ActionType::Idle, Some(&speed)), 1.0);
        assert_eq!(walk_speed_factor(ActionType::Attack, Some(&speed)), 1.0);
    }

    #[test]
    fn missing_speed_defaults_to_natural_rate() {
        assert_eq!(walk_speed_factor(ActionType::Walk, None), 1.0);
    }
}
