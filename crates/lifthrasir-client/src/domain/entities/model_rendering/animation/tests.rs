use crate::domain::entities::model_rendering::animation::*;
use lifthrasir_runtime::actors::Gr2Actor;
use std::time::Duration;

fn app(optional: bool) -> (App, Entity, Entity, [Option<ModelClip>; 5]) {
    let mut app = App::new();
    app.init_resource::<Time>().add_systems(
        Update,
        (
            crate::presentation::actors::sync_actor_animation_inputs,
            sync_animation,
        )
            .chain(),
    );
    let owner = app
        .world_mut()
        .spawn((
            AnimationState::Idle,
            Gr2Actor {
                model: "fixture.gr2".into(),
            },
        ))
        .id();
    let mut graph = AnimationGraph::new();
    let clips = std::array::from_fn(|i| {
        (i == 0 || optional).then(|| ModelClip {
            node: graph.add_clip(Handle::default(), 1.0, graph.root),
            duration: [1.0, 2.0, 3.0, 0.5, 2.0][i],
        })
    });
    let player = app
        .world_mut()
        .spawn((
            AnimationPlayer::default(),
            ActorPlayback {
                actor: owner,
                clips,
                current: None,
                request: None,
            },
        ))
        .id();
    (app, owner, player, clips)
}

#[test]
fn production_actor_phases_consume_same_frame_client_inputs_and_facing() {
    use crate::domain::system_sets::MovementSystems;
    use lifthrasir_runtime::actors::models::test_support::{ModelScene, ModelSceneLink};
    use lifthrasir_runtime::actors::sprites::legacy::{CharacterDirection, Direction};
    use lifthrasir_runtime::{ActorRenderPlugin, RuntimeCorePlugin};
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .add_plugins((
            lifthrasir_assets::ContentAssetsPlugin,
            RuntimeCorePlugin,
            ActorRenderPlugin,
        ));
    crate::presentation::actors::register_actor_input_bridge(&mut app);
    app.add_systems(
        Update,
        (|mut directions: Query<&mut CharacterDirection>| {
            for mut direction in &mut directions {
                direction.facing = Direction::West;
            }
        })
        .in_set(MovementSystems::TerrainAlignment),
    );
    let actor = app
        .world_mut()
        .spawn((
            Gr2Actor {
                model: "fixture.glb".into(),
            },
            AnimationState::Attacking,
            AttackTimer::new(0.5),
            CharacterDirection::default(),
        ))
        .id();
    let scene = app
        .world_mut()
        .spawn((
            ModelScene {
                model: "fixture.glb".into(),
                gltf: Handle::default(),
            },
            Transform::default(),
            ChildOf(actor),
        ))
        .id();
    app.world_mut()
        .entity_mut(actor)
        .insert(ModelSceneLink(scene));
    let mut graph = AnimationGraph::new();
    let clips = std::array::from_fn(|index| {
        Some(ModelClip {
            node: graph.add_clip(Handle::default(), 1.0, graph.root),
            duration: [1.0, 2.0, 3.0, 0.5, 2.0][index],
        })
    });
    let attack = clips[2].unwrap().node;
    let player = app
        .world_mut()
        .spawn((
            AnimationPlayer::default(),
            ActorPlayback {
                actor,
                clips,
                current: None,
                request: None,
            },
        ))
        .id();
    app.update();
    let animation = app
        .world()
        .get::<AnimationPlayer>(player)
        .unwrap()
        .animation(attack)
        .expect("the model must consume this frame's attack, not last frame's idle");
    assert_eq!(animation.speed(), 6.0);
    let rotation = app.world().get::<Transform>(scene).unwrap().rotation;
    assert!((rotation * Vec3::Z).abs_diff_eq(Vec3::NEG_X, 1e-5));
}

#[test]
fn model_attack_seeks_by_request_age_and_defaults_to_natural_rate_without_duration() {
    let (mut app, actor, player, clips) = app(true);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(200));
    app.world_mut()
        .entity_mut(actor)
        .remove::<AnimationState>()
        .insert(ActorAnimationInput {
            action: ActionType::Attack,
            restart_id: 1,
            started_at: Duration::from_millis(100),
            duration: Some(Duration::from_millis(500)),
            ..default()
        });
    app.update();
    let node = clips[2].unwrap().node;
    let animation = app
        .world()
        .get::<AnimationPlayer>(player)
        .unwrap()
        .animation(node)
        .unwrap();
    assert_eq!(animation.speed(), 6.0);
    assert!((animation.seek_time() - 0.6).abs() < 0.00001);
    app.world_mut()
        .get_mut::<ActorAnimationInput>(actor)
        .unwrap()
        .duration = None;
    app.update();
    let animation = app
        .world()
        .get::<AnimationPlayer>(player)
        .unwrap()
        .animation(node)
        .unwrap();
    assert_eq!(animation.speed(), 1.0);
    assert!((animation.seek_time() - 0.6).abs() < 0.00001);
}

#[test]
fn direct_inputs_restart_at_identical_times_and_can_leave_preview_death() {
    let (mut app, actor, player, clips) = app(true);
    app.world_mut()
        .entity_mut(actor)
        .remove::<AnimationState>()
        .insert(ActorAnimationInput {
            action: ActionType::Attack,
            restart_id: 1,
            duration: Some(Duration::from_millis(500)),
            ..default()
        });
    app.update();
    let attack = clips[2].unwrap().node;
    app.world_mut()
        .get_mut::<AnimationPlayer>(player)
        .unwrap()
        .animation_mut(attack)
        .unwrap()
        .seek_to(1.5);
    app.world_mut()
        .get_mut::<ActorAnimationInput>(actor)
        .unwrap()
        .restart_id = 2;
    app.update();
    assert_eq!(
        app.world()
            .get::<AnimationPlayer>(player)
            .unwrap()
            .animation(attack)
            .unwrap()
            .seek_time(),
        0.0
    );
    app.world_mut()
        .entity_mut(actor)
        .insert(ActorAnimationInput {
            action: ActionType::Dead,
            restart_id: 3,
            ..default()
        });
    app.update();
    app.world_mut()
        .entity_mut(actor)
        .insert(ActorAnimationInput {
            action: ActionType::Idle,
            restart_id: 4,
            ..default()
        });
    app.update();
    let idle = app
        .world()
        .get::<AnimationPlayer>(player)
        .unwrap()
        .animation(clips[0].unwrap().node)
        .unwrap();
    assert_eq!(idle.repeat_mode(), RepeatAnimation::Forever);
    assert!(!idle.is_paused());
}

#[test]
fn missing_non_attack_clips_keep_the_idle_fallback_progress() {
    let (mut app, actor, player, clips) = app(false);
    app.update();
    let idle = clips[0].unwrap().node;
    app.world_mut()
        .get_mut::<AnimationPlayer>(player)
        .unwrap()
        .animation_mut(idle)
        .unwrap()
        .seek_to(0.4);
    for state in [
        AnimationState::Walking,
        AnimationState::Hit,
        AnimationState::Dead,
    ] {
        app.world_mut().entity_mut(actor).insert(state);
        app.update();
        assert_eq!(
            app.world()
                .get::<AnimationPlayer>(player)
                .unwrap()
                .animation(idle)
                .unwrap()
                .seek_time(),
            0.4
        );
    }
}

#[test]
fn walking_scales_with_movement_and_death_is_not_looped() {
    let (mut app, actor, player, clips) = app(true);
    app.world_mut().entity_mut(actor).insert((
        AnimationState::Walking,
        MovementSpeed::from_server_speed(300),
    ));
    app.update();
    let animation = app
        .world()
        .get::<AnimationPlayer>(player)
        .unwrap()
        .animation(clips[1].unwrap().node)
        .unwrap();
    assert_eq!(animation.speed(), 0.5);
    assert_eq!(animation.repeat_mode(), RepeatAnimation::Forever);
    app.world_mut()
        .entity_mut(actor)
        .insert(AnimationState::Dead);
    app.update();
    let animation = app
        .world()
        .get::<AnimationPlayer>(player)
        .unwrap()
        .animation(clips[4].unwrap().node)
        .unwrap();
    assert_eq!(animation.repeat_mode(), RepeatAnimation::Never);
}

#[test]
fn repeated_attacks_restart_without_restarting_on_each_timer_tick() {
    let (mut app, actor, player, clips) = app(true);
    let mut timer = AttackTimer::new(0.5);
    timer.timer.tick(Duration::from_secs_f32(0.1));
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f32(0.1));
    app.world_mut()
        .entity_mut(actor)
        .insert((AnimationState::Attacking, timer));
    app.update();
    let node = clips[2].unwrap().node;
    assert_eq!(
        app.world()
            .get::<AnimationPlayer>(player)
            .unwrap()
            .animation(node)
            .unwrap()
            .speed(),
        6.0
    );
    app.world_mut()
        .get_mut::<AnimationPlayer>(player)
        .unwrap()
        .animation_mut(node)
        .unwrap()
        .seek_to(1.5);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f32(0.1));
    app.world_mut()
        .get_mut::<AttackTimer>(actor)
        .unwrap()
        .timer
        .tick(Duration::from_secs_f32(0.1));
    app.update();
    assert_eq!(
        app.world()
            .get::<AnimationPlayer>(player)
            .unwrap()
            .animation(node)
            .unwrap()
            .seek_time(),
        1.5
    );
    app.world_mut()
        .entity_mut(actor)
        .insert(AttackTimer::new(0.5));
    app.update();
    assert_eq!(
        app.world()
            .get::<AnimationPlayer>(player)
            .unwrap()
            .animation(node)
            .unwrap()
            .seek_time(),
        0.0
    );
}

#[test]
fn pause_holds_the_current_pose_across_state_changes_then_resumes() {
    let (mut app, actor, player, clips) = app(true);
    app.update();
    let idle = clips[0].unwrap().node;
    app.world_mut()
        .get_mut::<AnimationPlayer>(player)
        .unwrap()
        .animation_mut(idle)
        .unwrap()
        .seek_to(0.4);
    app.world_mut()
        .entity_mut(actor)
        .insert((AnimationPaused { at_ms: 400 }, AnimationState::Hit));
    app.update();
    let animation = app
        .world()
        .get::<AnimationPlayer>(player)
        .unwrap()
        .animation(idle)
        .unwrap();
    assert!(animation.is_paused());
    assert_eq!(animation.seek_time(), 0.4);
    app.world_mut()
        .entity_mut(actor)
        .remove::<AnimationPaused>();
    app.update();
    assert!(
        !app.world()
            .get::<AnimationPlayer>(player)
            .unwrap()
            .animation(clips[3].unwrap().node)
            .unwrap()
            .is_paused()
    );
}

#[test]
fn missing_optional_clips_use_idle() {
    let (mut app, actor, player, clips) = app(false);
    for state in [
        AnimationState::Walking,
        AnimationState::Attacking,
        AnimationState::Hit,
        AnimationState::Dead,
    ] {
        app.world_mut().entity_mut(actor).insert(state);
        app.update();
        assert!(
            app.world()
                .get::<AnimationPlayer>(player)
                .unwrap()
                .is_playing_animation(clips[0].unwrap().node)
        );
    }
}
