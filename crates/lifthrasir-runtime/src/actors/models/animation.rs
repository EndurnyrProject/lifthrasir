//! Drive converted glTF clips from shared presentation requests.

use super::*;
use crate::actors::{ActionType, ActorAnimationInput};
use bevy::{animation::RepeatAnimation, gltf::Gltf};
use lifthrasir_data::gr2::{ATTACK, DEAD, HIT, IDLE, WALK};

const ACTIONS: [&str; 5] = [IDLE, WALK, ATTACK, HIT, DEAD];

#[derive(Clone, Copy)]
pub struct ModelClip {
    pub node: AnimationNodeIndex,
    pub duration: f32,
}

#[derive(Component, Clone)]
pub struct ActorPlayback {
    pub actor: Entity,
    pub clips: [Option<ModelClip>; 5],
    pub current: Option<usize>,
    pub request: Option<(u64, ActionType)>,
}

/// Build once per scene instance. Clips remain shared assets; playback belongs to the actor.
pub(super) fn playback(
    actor: Entity,
    path: &str,
    gltf: &Gltf,
    clips: &Assets<AnimationClip>,
    graphs: &mut Assets<AnimationGraph>,
) -> Result<Option<(ActorPlayback, AnimationGraphHandle)>, String> {
    if !gltf.named_animations.contains_key(IDLE) {
        return Err(format!("GR2 model '{path}' has no idle animation"));
    }
    let mut graph = AnimationGraph::new();
    let mut actions = [None; 5];
    for (index, name) in ACTIONS.iter().enumerate() {
        let Some(handle) = gltf.named_animations.get(*name) else {
            continue;
        };
        let Some(clip) = clips.get(handle) else {
            return Ok(None);
        };
        let duration = clip.duration();
        if !duration.is_finite() || duration <= 0.0 {
            return Err(format!("GR2 model '{path}' has invalid {name} duration"));
        }
        let node = graph.add_clip(handle.clone(), 1.0, graph.root);
        actions[index] = Some(ModelClip { node, duration });
    }
    Ok(Some((
        ActorPlayback {
            actor,
            clips: actions,
            current: None,
            request: None,
        },
        AnimationGraphHandle(graphs.add(graph)),
    )))
}

fn action(action: ActionType) -> Option<usize> {
    match action {
        ActionType::Idle => Some(0),
        ActionType::Walk => Some(1),
        ActionType::Attack | ActionType::Attack1 | ActionType::Attack2 => Some(2),
        ActionType::Hit => Some(3),
        ActionType::Dead => Some(4),
        _ => None,
    }
}

type ActorMotion<'w, 's> = Query<'w, 's, &'static ActorAnimationInput>;

pub fn sync_animation(
    time: Res<Time>,
    actors: ActorMotion,
    mut players: Query<(&mut AnimationPlayer, &mut ActorPlayback)>,
) {
    for (mut player, mut playback) in &mut players {
        let Ok(input) = actors.get(playback.actor) else {
            continue;
        };
        if input.paused && playback.current.is_some() {
            for (_, active) in player.playing_animations_mut() {
                active.pause();
            }
            continue;
        }
        let requested = action(input.action);
        let desired = requested.unwrap_or(0);
        let selected = if playback.clips[desired].is_some() {
            desired
        } else {
            0
        };
        let clip = playback.clips[selected].expect("idle validated when wiring the scene");
        let request = (input.restart_id, input.action);
        let replay = playback.request != Some(request)
            && requested.is_some_and(|desired| {
                selected == desired || (desired == 2 && input.duration.is_some())
            });
        let restart = playback.current != Some(selected) || replay;
        let speed = if selected == 2 {
            input
                .duration
                .filter(|duration| !duration.is_zero())
                .map_or(1.0, |duration| clip.duration / duration.as_secs_f32())
        } else if selected == 1 {
            1.0 / input.delay_factor()
        } else {
            1.0
        };
        if restart {
            player.stop_all();
            let active = player.play(clip.node);
            if selected == 2 {
                let elapsed = time
                    .elapsed()
                    .saturating_sub(input.started_at)
                    .as_secs_f32();
                active.seek_to((elapsed * speed).min(clip.duration));
            }
            playback.current = Some(selected);
        }
        let active = player
            .animation_mut(clip.node)
            .expect("current model animation is playing");
        active.set_speed(speed);
        active.set_repeat(if selected <= 1 && input.action != ActionType::Dead {
            RepeatAnimation::Forever
        } else {
            RepeatAnimation::Never
        });
        if input.paused {
            active.pause();
        } else {
            active.resume();
        }
        playback.request = Some(request);
    }
}
