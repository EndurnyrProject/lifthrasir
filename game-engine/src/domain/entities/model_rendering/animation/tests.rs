use super::*;
use std::time::Duration;

fn app(optional: bool) -> (App, Entity, Entity, [Option<ModelClip>; 5]) {
    let mut app = App::new();
    app.init_resource::<Time>()
        .add_systems(Update, sync_animation);
    let owner = app.world_mut().spawn(AnimationState::Idle).id();
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
                attack_start: None,
            },
        ))
        .id();
    (app, owner, player, clips)
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
