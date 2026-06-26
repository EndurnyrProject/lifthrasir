use super::animation::AnimationState;
use super::status_effects::StatusEffects;
use crate::domain::combat::components::DeadEntity;
use crate::domain::entities::sprite_rendering::{EffectType, StatusEffectVisualEvent};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use moonshine_behavior::prelude::*;

type CharactersWithChangedStatus<'w, 's> =
    Query<'w, 's, (Entity, &'static StatusEffects), Changed<StatusEffects>>;

#[auto_add_system(
    plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin,
    schedule = Update,
    config(after = handle_status_effect_state_changes)
)]
pub fn observe_status_effects_changes(
    mut effect_events: MessageWriter<StatusEffectVisualEvent>,
    changed_status: CharactersWithChangedStatus,
) {
    for (entity, status) in changed_status.iter() {
        emit_effect(
            entity,
            EffectType::Poison,
            status.poisoned,
            &mut effect_events,
        );
        emit_effect(entity, EffectType::Stun, status.stunned, &mut effect_events);
        emit_effect(
            entity,
            EffectType::Freeze,
            status.frozen,
            &mut effect_events,
        );
        emit_effect(
            entity,
            EffectType::Stone,
            status.petrified,
            &mut effect_events,
        );
        emit_effect(
            entity,
            EffectType::Sleep,
            status.sleeping,
            &mut effect_events,
        );
    }
}

fn emit_effect(
    entity: Entity,
    effect_type: EffectType,
    active: bool,
    events: &mut MessageWriter<StatusEffectVisualEvent>,
) {
    events.write(StatusEffectVisualEvent {
        character: entity,
        effect_type,
        add: active,
    });
}

type CharactersWithChangedStatusEffects<'w, 's> =
    Query<'w, 's, (Entity, &'static StatusEffects), (Changed<StatusEffects>, Without<DeadEntity>)>;

#[auto_add_system(
    plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin,
    schedule = Update
)]
pub fn handle_status_effect_state_changes(
    mut behaviors: Query<BehaviorMut<AnimationState>>,
    characters: CharactersWithChangedStatusEffects,
) {
    for (entity, status) in characters.iter() {
        let Ok(mut behavior) = behaviors.get_mut(entity) else {
            continue;
        };

        let current_state = *behavior.current();

        if status.dead {
            if current_state != AnimationState::Dead {
                behavior.reset();
                behavior.start(AnimationState::Dead);
            }
            continue;
        }

        // Play Dead cleared: stand back up. reset() bypasses the Dead terminal
        // filter; real corpses carry DeadEntity and are excluded by the query.
        if current_state == AnimationState::Dead {
            behavior.reset();
            continue;
        }

        if (status.stunned || status.frozen || status.petrified)
            && current_state != AnimationState::Hit
        {
            behavior.start(AnimationState::Hit);
            continue;
        }

        if status.sleeping && current_state != AnimationState::Idle {
            behavior.start(AnimationState::Idle);
        }
    }
}
