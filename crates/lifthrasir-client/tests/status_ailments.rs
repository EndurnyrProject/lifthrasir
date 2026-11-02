use bevy::prelude::*;
use lifthrasir_client::core::GameState;
use lifthrasir_client::domain::effects::ailment_visuals::{
    PendingAilments, ailment_visuals, clear_pending_ailments,
};
use lifthrasir_client::domain::entities::registry::EntityRegistry;
use lifthrasir_runtime::audio::events::PlaySkillSfx;
use lifthrasir_runtime::effects::overlays::{AnimationPaused, BodyStateTint, HealthStateTint};
use lifthrasir_runtime::effects::sprites::{EffectSprite, EffectSpriteAction};
use net_contract::events::{UnitEntered, UnitLeft, UnitStateChanged};

fn app() -> App {
    let mut app = App::new();
    app.add_message::<UnitStateChanged>()
        .add_message::<UnitEntered>()
        .add_message::<UnitLeft>()
        .add_message::<PlaySkillSfx>()
        .init_resource::<EntityRegistry>()
        .init_resource::<PendingAilments>()
        .add_systems(Update, ailment_visuals);
    app
}

fn register(app: &mut App, unit: Entity) {
    app.world_mut()
        .resource_mut::<EntityRegistry>()
        .register_entity(7, unit);
}

fn state(app: &mut App, body_state: u32, health_state: u32) {
    app.world_mut()
        .resource_mut::<Messages<UnitStateChanged>>()
        .write(UnitStateChanged {
            unit_id: 7,
            body_state,
            health_state,
            effect_state: 0,
            virtue: 0,
        });
}

fn sprites(app: &App, unit: Entity) -> Vec<(Entity, String)> {
    app.world()
        .get::<Children>(unit)
        .into_iter()
        .flatten()
        .filter_map(|child| {
            app.world()
                .get::<EffectSprite>(*child)
                .map(|sprite| (*child, sprite.path.clone()))
        })
        .collect()
}

fn sounds(app: &mut App) -> Vec<String> {
    app.world_mut()
        .resource_mut::<Messages<PlaySkillSfx>>()
        .drain()
        .map(|event| event.sound)
        .collect()
}

#[test]
fn stun_repeats_one_head_overlay_without_pausing_or_replaying_sound() {
    let mut app = app();
    let unit = app.world_mut().spawn_empty().id();
    register(&mut app, unit);
    state(&mut app, 3, 0);
    state(&mut app, 3, 0);
    app.update();
    let overlays = sprites(&app, unit);
    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].1, "이팩트/status-stun");
    assert_eq!(sounds(&mut app), ["_stun.wav"]);
    assert!(app.world().get::<AnimationPaused>(unit).is_none());
    assert!(app.world().get::<BodyStateTint>(unit).is_none());
    assert!(
        app.world()
            .get::<Transform>(overlays[0].0)
            .unwrap()
            .translation
            .y
            > 0.0
    );

    state(&mut app, 3, 0);
    app.update();
    assert_eq!(sprites(&app, unit), overlays);
    assert!(sounds(&mut app).is_empty());

    state(&mut app, 0, 0);
    app.update();
    assert!(sprites(&app, unit).is_empty());
    assert!(sounds(&mut app).is_empty());
}

#[test]
fn health_ailments_combine_and_clear_independently() {
    let mut app = app();
    let unit = app.world_mut().spawn_empty().id();
    register(&mut app, unit);
    state(&mut app, 0, 1);
    app.update();
    assert_eq!(
        app.world().get::<HealthStateTint>(unit),
        Some(&HealthStateTint(Color::srgb(0.9, 0.4, 0.8)))
    );
    assert!(sprites(&app, unit).is_empty());
    assert_eq!(sounds(&mut app), ["_poison.wav"]);

    state(&mut app, 0, 1 | 2 | 4);
    app.update();
    let color = app
        .world()
        .get::<HealthStateTint>(unit)
        .unwrap()
        .0
        .to_srgba();
    assert!((color.red - 0.45).abs() < 0.000001);
    assert!((color.green - 0.06).abs() < 0.000001);
    assert!((color.blue - 0.08).abs() < 0.000001);
    let overlays = sprites(&app, unit);
    assert_eq!(overlays.len(), 2);
    let curse = overlays
        .iter()
        .find(|(_, path)| path == "이팩트/status-curse")
        .unwrap()
        .0;
    assert_eq!(
        app.world()
            .get::<EffectSprite>(curse)
            .unwrap()
            .tint
            .to_srgba()
            .alpha,
        0.5
    );
    let silence = overlays
        .iter()
        .find(|(_, path)| path == "이팩트/emotion")
        .unwrap()
        .0;
    assert_eq!(
        app.world().get::<EffectSpriteAction>(silence).unwrap().0,
        10
    );
    let mut emitted = sounds(&mut app);
    emitted.sort();
    assert_eq!(emitted, ["_curse.wav", "_silence.wav"]);

    state(&mut app, 0, 1 | 2 | 4);
    app.update();
    assert_eq!(sprites(&app, unit), overlays);
    assert!(sounds(&mut app).is_empty());

    state(&mut app, 0, 4);
    app.update();
    assert!(app.world().get::<HealthStateTint>(unit).is_none());
    assert_eq!(sprites(&app, unit), [(silence, "이팩트/emotion".into())]);
    state(&mut app, 0, 0);
    app.update();
    assert!(sprites(&app, unit).is_empty());
    assert!(sounds(&mut app).is_empty());
}

fn entered(body_state: u32, health_state: u32) -> UnitEntered {
    UnitEntered {
        gid: 7,
        aid: 0,
        object_type: 0,
        job: 0,
        x: 0,
        y: 0,
        dir: 0,
        speed: 0,
        hp: 0,
        max_hp: 0,
        clevel: 0,
        body_state,
        health_state,
        effect_state: 0,
        virtue: 0,
        spirit_sphere_count: 0,
        head: 0,
        weapon: 0,
        shield: 0,
        accessory: 0,
        accessory2: 0,
        accessory3: 0,
        head_palette: 0,
        body_palette: 0,
        head_dir: 0,
        robe: 0,
        guild_id: 0,
        guild_name: String::new(),
        emblem_id: 0,
        sex: 0,
        is_boss: false,
        name: String::new(),
        display_size: 0,
        moving: false,
        dst_x: 0,
        dst_y: 0,
        move_start_time: 0,
    }
}

#[test]
fn spawn_snapshot_restores_ailments_and_actor_despawn_removes_attachments() {
    let mut app = app();
    let unit = app.world_mut().spawn_empty().id();
    register(&mut app, unit);
    app.world_mut()
        .resource_mut::<Messages<UnitEntered>>()
        .write(entered(3, 1 | 2 | 4));
    app.update();
    let overlays = sprites(&app, unit);
    assert_eq!(overlays.len(), 3);
    assert!(app.world().get::<HealthStateTint>(unit).is_some());
    app.world_mut().despawn(unit);
    for (overlay, _) in overlays {
        assert!(app.world().get_entity(overlay).is_err());
    }
}

#[test]
fn delayed_registration_keeps_latest_live_state_ahead_of_spawn_snapshot() {
    let mut app = app();
    state(&mut app, 3, 1);
    app.update();
    let unit = app.world_mut().spawn_empty().id();
    register(&mut app, unit);
    app.update();
    assert_eq!(sprites(&app, unit).len(), 1);
    assert!(app.world().get::<HealthStateTint>(unit).is_some());
    sounds(&mut app);

    app.world_mut()
        .resource_mut::<EntityRegistry>()
        .unregister_entity(unit);
    state(&mut app, 3, 1);
    app.update();
    state(&mut app, 0, 0);
    app.update();
    register(&mut app, unit);
    app.world_mut()
        .resource_mut::<Messages<UnitEntered>>()
        .write(entered(3, 1));
    app.update();
    assert!(sprites(&app, unit).is_empty());
    assert!(app.world().get::<HealthStateTint>(unit).is_none());
    assert!(sounds(&mut app).is_empty());
}

#[test]
fn unit_leave_and_map_exit_discard_unresolved_states() {
    for map_exit in [false, true] {
        let mut app = app();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .insert_state(GameState::InGame)
            .add_systems(OnExit(GameState::InGame), clear_pending_ailments);
        app.update();
        state(&mut app, 3, 1);
        app.update();
        if map_exit {
            app.world_mut()
                .resource_mut::<NextState<GameState>>()
                .set(GameState::Loading);
        } else {
            app.world_mut()
                .resource_mut::<Messages<UnitLeft>>()
                .write(UnitLeft { gid: 7, reason: 0 });
        }
        app.update();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, unit);
        app.update();
        assert!(sprites(&app, unit).is_empty());
        assert!(app.world().get::<HealthStateTint>(unit).is_none());
        assert!(sounds(&mut app).is_empty());
    }
}

#[test]
fn curse_follows_the_actors_display_direction_without_new_status_packets() {
    use lifthrasir_client::domain::effects::ailment_visuals::sync_curse_direction;
    use lifthrasir_runtime::actors::sprites::components::PlayerSprite;
    use lifthrasir_runtime::rendering::Direction;
    let mut app = app();
    app.add_systems(Update, sync_curse_direction.after(ailment_visuals));
    let mut sprite = PlayerSprite::default();
    sprite.direction = Direction::East;
    let unit = app.world_mut().spawn(sprite).id();
    register(&mut app, unit);
    state(&mut app, 0, 2);
    app.update();
    let curse = sprites(&app, unit)[0].0;
    assert_eq!(
        app.world().get::<EffectSpriteAction>(curse).map(|a| a.0),
        Some(6)
    );
    app.world_mut()
        .get_mut::<PlayerSprite>(unit)
        .unwrap()
        .direction = Direction::West;
    app.update();
    assert_eq!(
        app.world().get::<EffectSpriteAction>(curse).map(|a| a.0),
        Some(2)
    );
}

#[test]
#[ignore = "requires the non-versioned assets/lifthrasir.pak retail archive"]
fn native_status_assets_are_playable() {
    use std::{fs::File, io::Read};
    let file = File::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/lifthrasir.pak"
    ))
    .unwrap();
    let mut pak = zip::ZipArchive::new(file).unwrap();
    let mut read = |path: &str| {
        let mut bytes = Vec::new();
        pak.by_name(path)
            .unwrap_or_else(|error| panic!("{path}: {error}"))
            .read_to_end(&mut bytes)
            .unwrap();
        bytes
    };
    for (stem, action_index) in [("status-stun", 0), ("status-curse", 0), ("emotion", 10)] {
        let action =
            ro_formats::parse_act(&read(&format!("data/sprite/이팩트/{stem}.act"))).unwrap();
        let sprite =
            ro_formats::parse_spr(&read(&format!("data/sprite/이팩트/{stem}.spr"))).unwrap();
        assert!(!sprite.frames.is_empty(), "{stem}: empty sprite");
        let selected_actions = if stem == "status-curse" {
            0..8
        } else {
            action_index..action_index + 1
        };
        for index in selected_actions {
            let selected = &action.actions[index];
            assert!(
                selected.delay > 0.0 && !selected.animations.is_empty(),
                "{stem}: unplayable action {index}"
            );
            assert!(
                selected
                    .animations
                    .iter()
                    .any(|frame| !frame.layers.is_empty()),
                "{stem}: action {index} has no layers"
            );
        }
    }
    for sound in ["_stun.wav", "_poison.wav", "_curse.wav", "_silence.wav"] {
        assert!(
            read(&format!("data/wav/{sound}")).starts_with(b"RIFF"),
            "{sound}"
        );
    }
}
