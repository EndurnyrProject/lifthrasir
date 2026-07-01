use std::time::Duration;

use super::{
    components::{AttackTimer, DeadEntity, HasEndure, HitStun, PendingHitReaction},
    events::{CombatActionType, DamageDisplayType, DisplayDamageNumber},
};
use crate::domain::{
    entities::{
        character::{components::visual::CharacterDirection, states::AnimationState},
        registry::EntityRegistry,
    },
    input::LockedTarget,
    system_sets::CombatSystems,
};
use crate::utils::coordinates::Direction;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use moonshine_behavior::prelude::*;
use net_contract::events::{DamageReceived, UnitLeft};

// =============================================================================
// PHASE 0.2: UPDATED TO USE FLAT ENTITY STRUCTURE
// =============================================================================
// Removed SpriteObjectTree dependency - queries entity Transform directly.
// =============================================================================

#[auto_add_system(
    plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update,
    config(in_set = CombatSystems::ProcessActions)
)]
pub fn process_combat_actions(
    mut commands: Commands,
    mut combat_events: MessageReader<DamageReceived>,
    mut damage_display: MessageWriter<DisplayDamageNumber>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
    registry: Res<EntityRegistry>,
    transforms: Query<&Transform>,
) {
    let events: Vec<_> = combat_events.read().collect();

    for event in events.iter() {
        let action_type = CombatActionType::from(event.type_ as u8);
        let src_speed = event.src_speed as i32;
        let dmg_speed = event.dmg_speed as i32;

        // aesir identifies every in-game unit by char_id (the gid field), so
        // combat src/target ids resolve against the registry's char_id key.
        let src_entity = registry.get_entity(event.src_id);
        let target_entity = registry.get_entity(event.target_id);

        if action_type.is_damage() {
            if let Some(src) = src_entity {
                start_attack_animation(
                    &mut commands,
                    &mut behaviors,
                    &transforms,
                    src,
                    target_entity,
                    src_speed,
                );
            } else {
                warn!("No entity found for src_id: {}", event.src_id);
            }

            if let Some(target) = target_entity {
                // The reaction fires when the swing connects: src_speed is the
                // attacker's attack motion (amotion), capped like the original
                // client so slow weapons don't feel unresponsive. dmg_speed is
                // the target's damage motion (dmotion) and sets the flinch length.
                let delay_ms = src_speed.clamp(0, 450) as u64;

                commands.spawn(PendingHitReaction {
                    target,
                    damage: event.damage,
                    is_critical: action_type.is_critical(),
                    flinches: action_type.target_flinches() && dmg_speed > 0,
                    stun_secs: dmg_speed.max(0) as f32 / 1000.0,
                    timer: Timer::new(Duration::from_millis(delay_ms), TimerMode::Once),
                });
            } else {
                warn!("No entity found for target_id: {}", event.target_id);
            }
        } else if action_type == CombatActionType::LuckyDodge {
            if let Some(target) = target_entity {
                damage_display.write(DisplayDamageNumber {
                    entity: target,
                    amount: 0,
                    damage_type: DamageDisplayType::Miss,
                });
            }
        } else if matches!(
            action_type,
            CombatActionType::SitDown | CombatActionType::StandUp
        ) {
            // The server broadcasts sit/stand (incl. back to the actor itself),
            // so this drives both the local player and remote players.
            let Some(src) = src_entity else {
                warn!("No entity found for src_id: {}", event.src_id);
                continue;
            };

            let next = if action_type == CombatActionType::SitDown {
                AnimationState::Sitting
            } else {
                AnimationState::Idle
            };

            if let Ok(mut behavior) = behaviors.get_mut(src) {
                if *behavior.current() != next {
                    behavior.start(next);
                }
            }
        }
    }
}

pub(crate) fn start_attack_animation(
    commands: &mut Commands,
    behaviors: &mut Query<BehaviorMut<AnimationState>>,
    transforms: &Query<&Transform>,
    src: Entity,
    target_entity: Option<Entity>,
    src_speed: i32,
) {
    // The server sends the attack motion duration (amotion) in ms; floor it so
    // high-ASPD swings stay readable instead of collapsing to a single frame.
    const MIN_ATTACK_ANIM_MS: u32 = 150;
    let attack_duration_ms = if src_speed > 0 {
        (src_speed as u32).max(MIN_ATTACK_ANIM_MS)
    } else {
        500
    };

    if let Some(target) = target_entity {
        let src_transform = transforms.get(src);
        let target_transform = transforms.get(target);

        if let (Ok(src_t), Ok(target_t)) = (src_transform, target_transform) {
            let dx = target_t.translation.x - src_t.translation.x;
            let dz = target_t.translation.z - src_t.translation.z;
            let direction = Direction::from_movement_vector(dx, dz);

            commands
                .entity(src)
                .insert(CharacterDirection { facing: direction });
        }
    }

    commands
        .entity(src)
        .insert(AttackTimer::new(attack_duration_ms as f32 / 1000.0, 0));

    if let Ok(mut behavior) = behaviors.get_mut(src) {
        behavior.start(AnimationState::Attacking);
    }
}

/// Fires scheduled hit reactions once the attacker's swing connects:
/// shows the damage number and plays the target's flinch with the
/// server-provided damage motion duration.
#[auto_add_system(
    plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update,
    config(in_set = CombatSystems::HandleReactions)
)]
pub fn apply_pending_hit_reactions(
    mut commands: Commands,
    time: Res<Time>,
    mut damage_display: MessageWriter<DisplayDamageNumber>,
    mut pending: Query<(Entity, &mut PendingHitReaction)>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
    targets: Query<(Has<HasEndure>, Has<AttackTimer>, Has<DeadEntity>)>,
) {
    for (entity, mut reaction) in pending.iter_mut() {
        reaction.timer.tick(time.delta());

        if !reaction.timer.just_finished() {
            continue;
        }

        commands.entity(entity).despawn();

        let Ok((has_endure, has_attack_timer, is_dead)) = targets.get(reaction.target) else {
            continue;
        };

        let damage_type = match (reaction.damage > 0, reaction.is_critical) {
            (false, _) => DamageDisplayType::Miss,
            (true, true) => DamageDisplayType::Critical,
            (true, false) => DamageDisplayType::Normal,
        };

        damage_display.write(DisplayDamageNumber {
            entity: reaction.target,
            amount: reaction.damage.max(0),
            damage_type,
        });

        if is_dead || has_endure || reaction.damage <= 0 || !reaction.flinches {
            continue;
        }

        if let Ok(mut behavior) = behaviors.get_mut(reaction.target) {
            behavior.start(AnimationState::Hit);
        }

        // Attackers keep swinging: their AttackTimer brings them back to idle.
        // Re-inserting HitStun on an already stunned target restarts the flinch.
        if !has_attack_timer {
            commands
                .entity(reaction.target)
                .insert(HitStun::new(reaction.stun_secs));
        }
    }
}

type HitReactionQuery<'w, 's> = Query<
    'w,
    's,
    Entity,
    (
        With<AnimationState>,
        Without<HasEndure>,
        Without<AttackTimer>,
        Without<HitStun>,
    ),
>;

#[auto_add_system(
    plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update,
    config(in_set = CombatSystems::HandleReactions)
)]
pub fn handle_hit_reactions(
    mut commands: Commands,
    hit_entities: HitReactionQuery,
    animation_states: Query<&AnimationState>,
) {
    for entity in hit_entities.iter() {
        if let Ok(state) = animation_states.get(entity) {
            if *state == AnimationState::Hit {
                commands.entity(entity).insert(HitStun::new(0.3));
            }
        }
    }
}

#[auto_add_system(
    plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update,
    config(in_set = CombatSystems::UpdateTimers)
)]
pub fn update_attack_timers(
    mut commands: Commands,
    time: Res<Time>,
    mut attack_timers: Query<(Entity, &mut AttackTimer), Without<DeadEntity>>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
) {
    for (entity, mut timer) in attack_timers.iter_mut() {
        timer.timer.tick(time.delta());

        if timer.timer.just_finished() {
            commands.entity(entity).remove::<AttackTimer>();

            if let Ok(mut behavior) = behaviors.get_mut(entity) {
                behavior.start(AnimationState::Idle);
            }
        }
    }
}

#[auto_add_system(
    plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update,
    config(in_set = CombatSystems::UpdateTimers)
)]
pub fn update_hit_stun(
    mut commands: Commands,
    time: Res<Time>,
    mut hit_stun: Query<(Entity, &mut HitStun), Without<DeadEntity>>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
) {
    for (entity, mut stun) in hit_stun.iter_mut() {
        stun.timer.tick(time.delta());

        if stun.timer.just_finished() {
            commands.entity(entity).remove::<HitStun>();

            if let Ok(mut behavior) = behaviors.get_mut(entity) {
                behavior.start(AnimationState::Idle);
            }
        }
    }
}

#[auto_add_system(
    plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update,
    config(in_set = CombatSystems::HandleDeath)
)]
pub fn handle_death(
    mut commands: Commands,
    mut vanish_events: MessageReader<UnitLeft>,
    registry: Res<EntityRegistry>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
    mut locked_target: ResMut<LockedTarget>,
) {
    for event in vanish_events.read() {
        if event.reason != 1 {
            continue;
        }

        if locked_target.gid == Some(event.gid) {
            *locked_target = LockedTarget::default();
        }

        let Some(entity) = registry.get_entity(event.gid) else {
            continue;
        };

        commands.entity(entity).insert(DeadEntity);

        if let Ok(mut behavior) = behaviors.get_mut(entity) {
            behavior.reset();
            behavior.start(AnimationState::Dead);
        }
    }
}

#[auto_add_system(
    plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update,
    config(in_set = CombatSystems::SpawnDamageIndicators)
)]
pub fn spawn_damage_indicators(
    mut commands: Commands,
    mut damage_events: MessageReader<DisplayDamageNumber>,
    entities: Query<&Transform>,
) {
    for event in damage_events.read() {
        let Ok(transform) = entities.get(event.entity) else {
            continue;
        };

        let color = match event.damage_type {
            DamageDisplayType::Normal => Color::WHITE,
            DamageDisplayType::Critical => Color::srgb(1.0, 0.5, 0.0),
            DamageDisplayType::Miss => Color::srgb(0.7, 0.7, 0.7),
        };

        let text = if event.amount == 0 {
            "Miss".to_string()
        } else {
            event.amount.to_string()
        };

        let spawn_pos = transform.translation + Vec3::new(0.0, 2.0, 0.0);
        let velocity = Vec3::new(0.0, 1.0, 0.0);

        commands.spawn((
            Text2d::new(text),
            TextColor(color),
            TextFont {
                font_size: 24.0.into(),
                ..default()
            },
            Transform::from_translation(spawn_pos),
            super::components::DamageIndicator::new(velocity),
        ));
    }
}

#[auto_add_system(
    plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update,
    config(in_set = CombatSystems::AnimateDamageIndicators)
)]
pub fn animate_damage_indicators(
    mut commands: Commands,
    time: Res<Time>,
    mut indicators: Query<(
        Entity,
        &mut Transform,
        &mut super::components::DamageIndicator,
    )>,
) {
    for (entity, mut transform, mut indicator) in indicators.iter_mut() {
        indicator.lifetime.tick(time.delta());

        if indicator.lifetime.just_finished() {
            commands.entity(entity).despawn();
            continue;
        }

        transform.translation += indicator.velocity * time.delta_secs();

        let _alpha = 1.0 - indicator.lifetime.fraction();
        indicator.velocity.y -= 0.5 * time.delta_secs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn death_app() -> App {
        let mut app = App::new();
        app.init_resource::<EntityRegistry>()
            .init_resource::<LockedTarget>()
            .add_message::<UnitLeft>()
            .add_systems(Update, handle_death);
        app
    }

    #[test]
    fn death_of_locked_target_clears_lock() {
        let mut app = death_app();
        app.world_mut().resource_mut::<LockedTarget>().gid = Some(7);
        app.world_mut()
            .write_message(UnitLeft { gid: 7, reason: 1 });

        app.update();

        assert_eq!(app.world().resource::<LockedTarget>().gid, None);
    }

    #[test]
    fn death_of_other_unit_keeps_lock() {
        let mut app = death_app();
        app.world_mut().resource_mut::<LockedTarget>().gid = Some(7);
        app.world_mut()
            .write_message(UnitLeft { gid: 9, reason: 1 });

        app.update();

        assert_eq!(app.world().resource::<LockedTarget>().gid, Some(7));
    }
}
