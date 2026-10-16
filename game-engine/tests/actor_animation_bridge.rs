use bevy::prelude::*;
use lifthrasir_client::domain::combat::components::AttackTimer;
use lifthrasir_client::domain::effects::AnimationPaused;
use lifthrasir_client::domain::entities::character::states::AnimationState;
use lifthrasir_client::domain::entities::model_rendering::Gr2Actor;
use lifthrasir_client::domain::entities::movement::components::MovementSpeed;
use lifthrasir_client::domain::entities::sprite_rendering::components::PlayerSprite;
use lifthrasir_client::presentation::actors::sync_actor_animation_inputs;
use lifthrasir_runtime::actors::{ActionType, ActorAnimationInput};
use std::time::Duration;

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<Time>()
        .add_systems(Update, sync_actor_animation_inputs);
    app
}

#[test]
fn state_machine_updates_do_not_create_a_new_request_each_frame() {
    use moonshine_behavior::prelude::*;
    let mut app = App::new();
    app.init_resource::<Time>()
        .add_plugins(BehaviorPlugin::<AnimationState>::default())
        .add_systems(
            Update,
            (transition::<AnimationState>, sync_actor_animation_inputs).chain(),
        );
    let actor = app
        .world_mut()
        .spawn((
            AnimationState::Attacking,
            AttackTimer::new(0.6),
            PlayerSprite::default(),
        ))
        .id();
    app.update();
    let request = app
        .world()
        .get::<ActorAnimationInput>(actor)
        .unwrap()
        .restart_id;
    for _ in 0..5 {
        app.update();
        assert_eq!(
            app.world()
                .get::<ActorAnimationInput>(actor)
                .unwrap()
                .restart_id,
            request
        );
    }
}

#[test]
fn initial_sprite_idle_keeps_the_existing_global_clock_phase() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(5));
    let actor = app
        .world_mut()
        .spawn((AnimationState::Idle, PlayerSprite::default()))
        .id();
    app.update();
    assert_eq!(
        app.world()
            .get::<ActorAnimationInput>(actor)
            .unwrap()
            .started_at,
        Duration::ZERO
    );
}

#[test]
fn repeated_attacks_at_one_timestamp_restart_but_timer_ticks_do_not() {
    let mut app = app();
    let actor = app
        .world_mut()
        .spawn((
            AnimationState::Attacking,
            AttackTimer::new(0.6),
            PlayerSprite::default(),
        ))
        .id();
    app.update();
    let first = *app.world().get::<ActorAnimationInput>(actor).unwrap();
    app.world_mut()
        .entity_mut(actor)
        .insert(AttackTimer::new(0.6));
    app.update();
    let second = *app.world().get::<ActorAnimationInput>(actor).unwrap();
    assert!(second.restart_id > first.restart_id);
    assert_eq!(second.started_at, first.started_at);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(100));
    app.world_mut()
        .get_mut::<AttackTimer>(actor)
        .unwrap()
        .timer
        .tick(Duration::from_millis(100));
    app.update();
    assert_eq!(
        app.world()
            .get::<ActorAnimationInput>(actor)
            .unwrap()
            .restart_id,
        second.restart_id
    );
}

#[test]
fn walk_rate_and_pause_changes_do_not_restart_an_existing_request() {
    let mut app = app();
    let actor = app
        .world_mut()
        .spawn((AnimationState::Walking, PlayerSprite::default()))
        .id();
    app.update();
    let request = *app.world().get::<ActorAnimationInput>(actor).unwrap();
    assert_eq!(request.speed_factor, 1.0);
    app.world_mut()
        .entity_mut(actor)
        .insert(MovementSpeed::from_server_speed(300));
    app.update();
    let faster = app.world().get::<ActorAnimationInput>(actor).unwrap();
    assert_eq!(faster.restart_id, request.restart_id);
    assert_eq!(faster.speed_factor, 2.0);
    app.world_mut()
        .entity_mut(actor)
        .insert(AnimationPaused { at_ms: 100 });
    app.update();
    let paused = app.world().get::<ActorAnimationInput>(actor).unwrap();
    assert!(paused.paused);
    assert_eq!(paused.restart_id, request.restart_id);
}

#[test]
fn frozen_pose_defers_state_changes_and_resumes_the_latest_request() {
    let mut app = app();
    let actor = app
        .world_mut()
        .spawn((
            AnimationState::Idle,
            PlayerSprite::default(),
            AnimationPaused { at_ms: 500 },
        ))
        .id();
    app.update();
    let first = *app.world().get::<ActorAnimationInput>(actor).unwrap();
    *app.world_mut().get_mut::<AnimationState>(actor).unwrap() = AnimationState::Hit;
    app.update();
    assert_eq!(
        app.world()
            .get::<ActorAnimationInput>(actor)
            .unwrap()
            .action,
        ActionType::Idle
    );
    assert_eq!(
        app.world()
            .get::<ActorAnimationInput>(actor)
            .unwrap()
            .restart_id,
        first.restart_id
    );
    app.world_mut()
        .entity_mut(actor)
        .remove::<AnimationPaused>();
    app.update();
    let resumed = app.world().get::<ActorAnimationInput>(actor).unwrap();
    assert_eq!(resumed.action, ActionType::Hit);
    assert!(!resumed.paused);
    assert!(resumed.restart_id > first.restart_id);
}

#[test]
fn bridge_preserves_sprite_start_and_model_attack_seek_calibration() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(200));
    let mut timer = AttackTimer::new(0.5);
    timer.timer.tick(Duration::from_millis(100));
    let sprite = app
        .world_mut()
        .spawn((
            AnimationState::Attacking,
            timer.clone(),
            PlayerSprite::default(),
        ))
        .id();
    let model = app
        .world_mut()
        .spawn((
            AnimationState::Attacking,
            timer,
            Gr2Actor {
                model: "fixture.gr2".into(),
            },
        ))
        .id();
    app.update();
    let sprite_input = app.world().get::<ActorAnimationInput>(sprite).unwrap();
    let model_input = app.world().get::<ActorAnimationInput>(model).unwrap();
    assert_eq!(sprite_input.started_at, Duration::from_millis(200));
    assert_eq!(model_input.started_at, Duration::from_millis(100));
    assert_eq!(sprite_input.duration, Some(Duration::from_millis(500)));
    assert_eq!(model_input.duration, sprite_input.duration);
}
