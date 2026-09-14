//! Drive glTF animation from the same state and timing components used by sprites.

use super::*;
use crate::domain::{
    combat::components::AttackTimer,
    effects::AnimationPaused,
    entities::{character::states::AnimationState, movement::components::MovementSpeed},
};
use bevy::{animation::RepeatAnimation, gltf::Gltf};
use lifthrasir_data::gr2::{ATTACK, DEAD, HIT, IDLE, WALK};

const ACTIONS: [&str; 5] = [IDLE, WALK, ATTACK, HIT, DEAD];

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(super) struct ModelClip {
    pub node: AnimationNodeIndex,
    pub duration: f32,
}

#[derive(Component)]
pub(super) struct ActorPlayback {
    pub actor: Entity,
    pub clips: [Option<ModelClip>; 5],
    pub current: Option<usize>,
    pub attack_start: Option<f64>,
}

/// Build once per scene instance. Clips remain shared assets; playback belongs to the actor.
pub(super) fn playback(
    actor: Entity,
    path: &str,
    gltf: &Gltf,
    clips: &Assets<AnimationClip>,
    graphs: &mut Assets<AnimationGraph>,
) -> Option<(ActorPlayback, AnimationGraphHandle)> {
    assert!(
        gltf.named_animations.contains_key(IDLE),
        "GR2 model '{path}' has no idle animation"
    );
    let mut graph = AnimationGraph::new();
    let mut actions = [None; 5];
    for (index, name) in ACTIONS.iter().enumerate() {
        let Some(handle) = gltf.named_animations.get(*name) else {
            continue;
        };
        let clip = clips.get(handle)?;
        let duration = clip.duration();
        assert!(
            duration.is_finite() && duration > 0.0,
            "GR2 model '{path}' has invalid {name} duration"
        );
        let node = graph.add_clip(handle.clone(), 1.0, graph.root);
        actions[index] = Some(ModelClip { node, duration });
    }
    Some((
        ActorPlayback {
            actor,
            clips: actions,
            current: None,
            attack_start: None,
        },
        AnimationGraphHandle(graphs.add(graph)),
    ))
}

fn action(state: AnimationState) -> usize {
    match state {
        AnimationState::Walking => 1,
        AnimationState::Attacking => 2,
        AnimationState::Hit => 3,
        AnimationState::Dead => 4,
        _ => 0,
    }
}

type ActorMotion<'w, 's> = Query<
    'w,
    's,
    (
        &'static AnimationState,
        Option<&'static AttackTimer>,
        Option<&'static MovementSpeed>,
        Has<AnimationPaused>,
    ),
>;

pub(super) fn sync_animation(
    time: Res<Time>,
    actors: ActorMotion,
    mut players: Query<(&mut AnimationPlayer, &mut ActorPlayback)>,
) {
    for (mut player, mut playback) in &mut players {
        let Ok((state, timer, movement, paused)) = actors.get(playback.actor) else {
            continue;
        };
        if paused && playback.current.is_some() {
            for (_, active) in player.playing_animations_mut() {
                active.pause();
            }
            continue;
        }
        let desired = action(*state);
        let selected = if playback.clips[desired].is_some() {
            desired
        } else {
            0
        };
        let clip = playback.clips[selected].expect("idle validated when wiring the scene");
        let attack_start = timer
            .filter(|_| desired == 2)
            .map(|t| time.elapsed_secs_f64() - t.timer.elapsed().as_secs_f64());
        let new_attack = attack_start.is_some_and(|start| {
            playback
                .attack_start
                .is_none_or(|previous| (start - previous).abs() > 0.0001)
        });
        let restart = playback.current != Some(selected) || new_attack;
        let speed = if selected == 2 {
            timer
                .filter(|t| t.timer.duration().as_secs_f32() > 0.0)
                .map_or(1.0, |t| clip.duration / t.timer.duration().as_secs_f32())
        } else if selected == 1 {
            movement
                .filter(|m| m.ms_per_cell.is_finite() && m.ms_per_cell > 0.0)
                .map_or(1.0, |m| 150.0 / m.ms_per_cell)
        } else {
            1.0
        };
        if restart {
            player.stop_all();
            let active = player.play(clip.node);
            if selected == 2
                && let Some(timer) = timer
            {
                active.seek_to((timer.timer.elapsed_secs() * speed).min(clip.duration));
            }
            playback.current = Some(selected);
        }
        let active = player
            .animation_mut(clip.node)
            .expect("current model animation is playing");
        active.set_speed(speed);
        active.set_repeat(if selected <= 1 && *state != AnimationState::Dead {
            RepeatAnimation::Forever
        } else {
            RepeatAnimation::Never
        });
        if paused {
            active.pause();
        } else {
            active.resume();
        }
        playback.attack_start = attack_start;
    }
}
