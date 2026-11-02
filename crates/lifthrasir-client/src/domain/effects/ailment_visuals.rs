//! State-driven presentation for stun, poison, curse and silence.
//!
//! Stun/curse sprites and poison/curse tints follow roBrowser's EntityState.
//! Silence follows roBrowserLegacy's EntityState and Emotions table: action 10
//! of emotion.act is the looping "..." bubble, not the silence-attack STR.
//! Reference: https://github.com/MrAntares/roBrowserLegacy/blob/master/src/Renderer/Entity/EntityState.js

use std::collections::HashMap;

use bevy::prelude::*;
use net_contract::events::{UnitEntered, UnitLeft, UnitStateChanged};

use crate::domain::entities::registry::EntityRegistry;
use lifthrasir_runtime::actors::sprites::components::{MobSprite, PlayerSprite};
use lifthrasir_runtime::audio::events::PlaySkillSfx;
use lifthrasir_runtime::effects::overlays::{HealthStateTint, multiply_colors};
use lifthrasir_runtime::effects::sprites::{EffectSprite, EffectSpriteAction};

const OPT1_STUN: u32 = 3;
const OPT2_POISON: u32 = 1;
const OPT2_CURSE: u32 = 2;
const OPT2_SILENCE: u32 = 4;
/// Match the existing emote renderer's above-head offset.
const ABOVE_HEAD_Y: f32 = 12.0;

/// Latest opt1/opt2 state received before a unit's entity registers.
#[derive(Resource, Default)]
pub struct PendingAilments(HashMap<u32, (u32, u32)>);

#[doc(hidden)]
#[derive(Component, Default, Clone)]
pub struct AppliedAilments {
    stun: Option<Entity>,
    curse: Option<Entity>,
    silence: Option<Entity>,
    health_state: u32,
}

/// Discard unresolved unit IDs when leaving the map that supplied them.
pub fn clear_pending_ailments(mut pending: ResMut<PendingAilments>) {
    pending.0.clear();
}

/// Reconcile snapshots and live updates once per unit, so repeated messages in
/// one frame cannot duplicate deferred spawns. Live changes supersede snapshots.
#[allow(clippy::too_many_arguments)]
pub fn ailment_visuals(
    mut entered: MessageReader<UnitEntered>,
    mut changed: MessageReader<UnitStateChanged>,
    mut left: MessageReader<UnitLeft>,
    registry: Res<EntityRegistry>,
    mut pending: ResMut<PendingAilments>,
    applied: Query<&AppliedAilments>,
    mut commands: Commands,
    mut sfx: MessageWriter<PlaySkillSfx>,
) {
    for event in entered.read() {
        pending
            .0
            .entry(event.gid)
            .or_insert((event.body_state, event.health_state));
    }
    for event in changed.read() {
        pending
            .0
            .insert(event.unit_id, (event.body_state, event.health_state));
    }
    for event in left.read() {
        pending.0.remove(&event.gid);
    }

    pending.0.retain(|&gid, &mut (body_state, health_state)| {
        let Some(unit) = registry.get_entity(gid) else {
            return true;
        };
        let mut visual = applied.get(unit).cloned().unwrap_or_default();
        for (active, overlay, path, tint, sound) in [
            (
                body_state == OPT1_STUN,
                &mut visual.stun,
                "이팩트/status-stun",
                Color::WHITE,
                "_stun.wav",
            ),
            (
                health_state & OPT2_CURSE != 0,
                &mut visual.curse,
                "이팩트/status-curse",
                Color::srgba(1.0, 1.0, 1.0, 0.5),
                "_curse.wav",
            ),
            (
                health_state & OPT2_SILENCE != 0,
                &mut visual.silence,
                "이팩트/emotion",
                Color::WHITE,
                "_silence.wav",
            ),
        ] {
            match (active, *overlay) {
                (true, None) => {
                    *overlay = Some(spawn_head_sprite(&mut commands, unit, path, tint));
                    sfx.write(PlaySkillSfx {
                        emitter: unit,
                        sound: sound.into(),
                    });
                }
                (false, Some(entity)) => {
                    commands.entity(entity).despawn();
                    *overlay = None;
                }
                _ => {}
            }
        }
        if let Some(silence) = visual.silence {
            commands.entity(silence).insert(EffectSpriteAction(10));
        }
        if health_state & OPT2_POISON != 0 && visual.health_state & OPT2_POISON == 0 {
            sfx.write(PlaySkillSfx {
                emitter: unit,
                sound: "_poison.wav".into(),
            });
        }
        match health_state_tint(health_state) {
            Some(tint) => {
                commands.entity(unit).insert(HealthStateTint(tint));
            }
            None => {
                commands.entity(unit).remove::<HealthStateTint>();
            }
        }
        visual.health_state = health_state;
        commands.entity(unit).insert(visual);
        false
    });
}

/// Keeps the curse attachment on the actor's camera-relative ACT direction.
pub fn sync_curse_direction(
    units: Query<(&AppliedAilments, Option<&PlayerSprite>, Option<&MobSprite>)>,
    actions: Query<&EffectSpriteAction>,
    mut commands: Commands,
) {
    for (visual, player, mob) in &units {
        let Some(curse) = visual.curse else {
            continue;
        };
        let Some(direction) = player
            .map(|sprite| sprite.direction)
            .or_else(|| mob.map(|sprite| sprite.direction))
        else {
            continue;
        };
        let action = direction as usize;
        if actions.get(curse).is_ok_and(|current| current.0 == action) {
            continue;
        }
        commands.entity(curse).insert(EffectSpriteAction(action));
    }
}

fn health_state_tint(health_state: u32) -> Option<Color> {
    [
        (OPT2_POISON, Color::srgb(0.9, 0.4, 0.8)),
        (OPT2_CURSE, Color::srgb(0.5, 0.15, 0.1)),
    ]
    .into_iter()
    .filter_map(|(bit, tint)| (health_state & bit != 0).then_some(tint))
    .reduce(multiply_colors)
}

fn spawn_head_sprite(commands: &mut Commands, unit: Entity, path: &str, tint: Color) -> Entity {
    commands
        .spawn((
            EffectSprite {
                path: path.into(),
                tint,
            },
            Transform::from_xyz(0.0, ABOVE_HEAD_Y, 0.0),
            Visibility::Inherited,
            Pickable::IGNORE,
            ChildOf(unit),
        ))
        .id()
}
