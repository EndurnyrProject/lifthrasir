use std::time::Duration;

use super::{
    components::{AttackTimer, DeadEntity, HasEndure, HitStun, PendingHitReaction},
    events::{CombatActionType, DamageDisplayType, DisplayDamageNumber},
};
use crate::domain::{
    entities::{
        character::{components::visual::CharacterDirection, states::AnimationState},
        markers::LocalPlayer,
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
    for event in combat_events.read() {
        let action_type = CombatActionType::from(event.type_ as u8);
        match action_type {
            action if action.is_damage() => process_damage_action(
                &mut commands,
                &mut behaviors,
                &registry,
                &transforms,
                event,
                action,
            ),
            CombatActionType::LuckyDodge => {
                display_lucky_dodge(&mut damage_display, &registry, event)
            }
            CombatActionType::SitDown | CombatActionType::StandUp => {
                process_posture_action(&mut behaviors, &registry, event, action_type)
            }
            _ => {}
        }
    }
}

fn process_damage_action(
    commands: &mut Commands,
    behaviors: &mut Query<BehaviorMut<AnimationState>>,
    registry: &EntityRegistry,
    transforms: &Query<&Transform>,
    event: &DamageReceived,
    action_type: CombatActionType,
) {
    let src_speed = event.src_speed as i32;
    let target_entity = registry.get_entity(event.target_id);

    if let Some(src) = registry.get_entity(event.src_id) {
        start_attack_animation(
            commands,
            behaviors,
            transforms,
            src,
            target_entity,
            src_speed,
        );
    } else {
        warn!("No entity found for src_id: {}", event.src_id);
    }

    let Some(target) = target_entity else {
        warn!("No entity found for target_id: {}", event.target_id);
        return;
    };

    // The reaction fires when the swing connects: src_speed is the attacker's
    // attack motion (amotion), capped so slow weapons stay responsive. dmg_speed
    // is the target's damage motion (dmotion) and sets the flinch length.
    let dmg_speed = event.dmg_speed as i32;
    let delay_ms = src_speed.clamp(0, 450) as u64;
    commands.spawn(PendingHitReaction {
        target,
        damage: event.damage,
        is_critical: action_type.is_critical(),
        flinches: action_type.target_flinches() && dmg_speed > 0,
        stun_secs: dmg_speed.max(0) as f32 / 1000.0,
        timer: Timer::new(Duration::from_millis(delay_ms), TimerMode::Once),
    });
}

fn display_lucky_dodge(
    damage_display: &mut MessageWriter<DisplayDamageNumber>,
    registry: &EntityRegistry,
    event: &DamageReceived,
) {
    let Some(target) = registry.get_entity(event.target_id) else {
        return;
    };

    damage_display.write(DisplayDamageNumber {
        entity: target,
        amount: 0,
        damage_type: DamageDisplayType::Miss,
        delay_secs: 0.0,
    });
}

fn process_posture_action(
    behaviors: &mut Query<BehaviorMut<AnimationState>>,
    registry: &EntityRegistry,
    event: &DamageReceived,
    action_type: CombatActionType,
) {
    // The server broadcasts sit/stand back to the actor, so this drives both
    // the local player and remote players.
    let Some(src) = registry.get_entity(event.src_id) else {
        warn!("No entity found for src_id: {}", event.src_id);
        return;
    };

    let next = match action_type {
        CombatActionType::SitDown => AnimationState::Sitting,
        CombatActionType::StandUp => AnimationState::Idle,
        _ => return,
    };

    let Ok(mut behavior) = behaviors.get_mut(src) else {
        return;
    };
    if *behavior.current() != next {
        behavior.start(next);
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
        .insert(AttackTimer::new(attack_duration_ms as f32 / 1000.0));

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
            delay_secs: 0.0,
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
    locked_target: Res<LockedTarget>,
    mut attack_timers: Query<(Entity, &mut AttackTimer, Has<LocalPlayer>), Without<DeadEntity>>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
) {
    for (entity, mut timer, is_local_player) in attack_timers.iter_mut() {
        timer.timer.tick(time.delta());

        if timer.timer.just_finished() {
            commands.entity(entity).remove::<AttackTimer>();

            let next = if is_local_player && locked_target.gid.is_some() {
                AnimationState::CombatReady
            } else {
                AnimationState::Idle
            };

            if let Ok(mut behavior) = behaviors.get_mut(entity) {
                behavior.start(next);
            }
        }
    }
}

#[auto_add_system(
    plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update,
    config(in_set = CombatSystems::UpdateTimers, before = update_attack_timers)
)]
pub fn drive_combat_ready_pose(
    locked_target: Res<LockedTarget>,
    mut player: Query<(BehaviorMut<AnimationState>, Has<AttackTimer>), With<LocalPlayer>>,
) {
    let Ok((mut behavior, has_attack_timer)) = player.single_mut() else {
        return;
    };

    if locked_target.gid.is_none() {
        if *behavior.current() == AnimationState::CombatReady {
            behavior.start(AnimationState::Idle);
        }
        return;
    }

    if has_attack_timer {
        return;
    }

    if *behavior.current() == AnimationState::Idle {
        behavior.start(AnimationState::CombatReady);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn combat_action_app() -> App {
        let mut app = App::new();
        app.add_plugins(BehaviorPlugin::<AnimationState>::default())
            .init_resource::<EntityRegistry>()
            .add_message::<DamageReceived>()
            .add_message::<DisplayDamageNumber>()
            .add_systems(
                Update,
                (process_combat_actions, transition::<AnimationState>).chain(),
            );
        app
    }

    fn spawn_registered(app: &mut App, gid: u32, transform: Transform) -> Entity {
        let entity = app
            .world_mut()
            .spawn((transform, AnimationState::Idle))
            .id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(gid, entity);
        entity
    }

    fn combat_action(type_: u32) -> DamageReceived {
        DamageReceived {
            src_id: 1,
            target_id: 2,
            server_tick: 0,
            src_speed: 200,
            dmg_speed: 300,
            damage: 42,
            div: 1,
            type_,
            damage2: 0,
        }
    }

    #[test]
    fn damage_action_starts_attack_and_schedules_target_reaction() {
        let mut app = combat_action_app();
        let source = spawn_registered(&mut app, 1, Transform::default());
        let target = spawn_registered(&mut app, 2, Transform::from_xyz(1.0, 0.0, 0.0));

        app.world_mut().write_message(combat_action(0));
        app.update();

        assert_eq!(state(&app, source), AnimationState::Attacking);
        let attack_duration = app
            .world()
            .get::<AttackTimer>(source)
            .unwrap()
            .timer
            .duration()
            .as_secs_f32();
        assert!((attack_duration - 0.2).abs() < f32::EPSILON);
        assert!(app.world().get::<CharacterDirection>(source).is_some());

        let world = app.world_mut();
        let mut reactions = world.query::<&PendingHitReaction>();
        let reaction = reactions.single(world).unwrap();
        assert_eq!(reaction.target, target);
        assert_eq!(reaction.damage, 42);
        assert!(!reaction.is_critical);
        assert!(reaction.flinches);
        assert_eq!(reaction.stun_secs, 0.3);
        assert_eq!(reaction.timer.duration(), Duration::from_millis(200));
    }

    #[test]
    fn lucky_dodge_displays_an_immediate_miss() {
        let mut app = combat_action_app();
        let target = spawn_registered(&mut app, 2, Transform::default());

        app.world_mut().write_message(combat_action(11));
        app.update();

        let messages: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<DisplayDamageNumber>>()
            .drain()
            .collect();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].entity, target);
        assert_eq!(messages[0].amount, 0);
        assert_eq!(messages[0].damage_type, DamageDisplayType::Miss);
        assert_eq!(messages[0].delay_secs, 0.0);
    }

    #[test]
    fn posture_actions_toggle_sitting_state() {
        let mut app = combat_action_app();
        let source = spawn_registered(&mut app, 1, Transform::default());

        app.world_mut().write_message(combat_action(2));
        app.update();
        assert_eq!(state(&app, source), AnimationState::Sitting);

        app.world_mut().write_message(combat_action(3));
        app.update();
        assert_eq!(state(&app, source), AnimationState::Idle);
    }

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

    fn combat_ready_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin)
            .add_plugins(BehaviorPlugin::<AnimationState>::default())
            .init_resource::<LockedTarget>()
            .add_systems(
                Update,
                (
                    drive_combat_ready_pose,
                    update_attack_timers,
                    transition::<AnimationState>,
                )
                    .chain(),
            );
        app
    }

    fn state(app: &App, entity: Entity) -> AnimationState {
        *app.world().get::<AnimationState>(entity).unwrap()
    }

    #[test]
    fn locked_idle_local_player_enters_combat_ready() {
        let mut app = combat_ready_app();
        app.world_mut().resource_mut::<LockedTarget>().gid = Some(1);
        let player = app
            .world_mut()
            .spawn((LocalPlayer, AnimationState::Idle))
            .id();

        app.update();

        assert_eq!(state(&app, player), AnimationState::CombatReady);
    }

    #[test]
    fn swing_end_returns_local_player_to_combat_ready_when_locked() {
        let mut app = combat_ready_app();
        app.world_mut().resource_mut::<LockedTarget>().gid = Some(1);
        let player = app
            .world_mut()
            .spawn((
                LocalPlayer,
                AnimationState::Attacking,
                AttackTimer::new(0.0),
            ))
            .id();

        app.update();

        assert!(app.world().get::<AttackTimer>(player).is_none());
        assert_eq!(state(&app, player), AnimationState::CombatReady);
    }

    #[test]
    fn swing_end_returns_local_player_to_idle_when_not_locked() {
        let mut app = combat_ready_app();
        let player = app
            .world_mut()
            .spawn((
                LocalPlayer,
                AnimationState::Attacking,
                AttackTimer::new(0.0),
            ))
            .id();

        app.update();

        assert_eq!(state(&app, player), AnimationState::Idle);
    }

    #[test]
    fn swing_end_never_gives_combat_ready_to_non_local_entity() {
        let mut app = combat_ready_app();
        app.world_mut().resource_mut::<LockedTarget>().gid = Some(1);
        let mob = app
            .world_mut()
            .spawn((AnimationState::Attacking, AttackTimer::new(0.0)))
            .id();

        app.update();

        assert_eq!(state(&app, mob), AnimationState::Idle);
    }

    #[test]
    fn drive_combat_ready_pose_leaves_busy_state_alone() {
        let mut app = combat_ready_app();
        app.world_mut().resource_mut::<LockedTarget>().gid = Some(1);
        let player = app
            .world_mut()
            .spawn((LocalPlayer, AnimationState::Attacking))
            .id();

        app.update();

        assert_eq!(state(&app, player), AnimationState::Attacking);
    }

    #[test]
    fn clearing_lock_returns_local_player_to_idle() {
        let mut app = combat_ready_app();
        app.world_mut().resource_mut::<LockedTarget>().gid = Some(1);
        let player = app
            .world_mut()
            .spawn((LocalPlayer, AnimationState::Idle))
            .id();

        app.update();
        assert_eq!(state(&app, player), AnimationState::CombatReady);

        app.world_mut().resource_mut::<LockedTarget>().gid = None;
        app.update();

        assert_eq!(state(&app, player), AnimationState::Idle);
    }
}
