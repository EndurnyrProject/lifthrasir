use bevy::prelude::*;
use bevy_kira_audio::{
    Audio, AudioChannel, AudioControl, AudioInstance, AudioSource, PlaybackState,
    SpatialAudioEmitter,
};
use lifthrasir_runtime::audio::{
    AmbienceChannel, AudioSettings, BgmManager, ContentAudioListener, SfxChannel,
};
use lifthrasir_runtime::audio::{events::*, playback::*};

fn playback_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<AudioSource>()
        .init_asset::<AudioInstance>()
        .add_plugins(lifthrasir_runtime::RuntimeCorePlugin)
        .init_resource::<BgmManager>()
        .init_resource::<AudioSettings>()
        .init_resource::<Audio>()
        .init_resource::<AudioChannel<SfxChannel>>()
        .init_resource::<AudioChannel<AmbienceChannel>>()
        .add_systems(
            Update,
            (
                handle_bgm_change,
                handle_bgm_stop,
                play_mob_sfx,
                play_skill_sfx,
                handle_volume_change,
                handle_mute_change,
                handle_sfx_volume_change,
                handle_sfx_mute_change,
                handle_ambience_volume_change,
                handle_ambience_mute_change,
            ),
        );
    app
}

#[test]
fn sfx_requests_attach_to_explicit_entities_without_client_markers() {
    let mut app = playback_app();
    let listener = app
        .world_mut()
        .spawn((ContentAudioListener, bevy_kira_audio::SpatialAudioReceiver))
        .id();
    let skill = app.world_mut().spawn_empty().id();
    let mob = app.world_mut().spawn(SpatialAudioEmitter::default()).id();
    app.world_mut().write_message(PlaySkillSfx {
        emitter: skill,
        sound: "effect\\skill.wav".into(),
    });
    app.world_mut().write_message(PlayMobSfx {
        emitter: mob,
        sound: "poring.wav".into(),
    });
    app.update();
    assert!(app.world().get::<ContentAudioListener>(listener).is_some());
    for entity in [skill, mob] {
        let emitter = app.world().get::<SpatialAudioEmitter>(entity).unwrap();
        assert_eq!(emitter.instances.len(), 1);
        assert_eq!(
            app.world()
                .resource::<AudioChannel<SfxChannel>>()
                .state(&emitter.instances[0]),
            PlaybackState::Queued
        );
    }
}

#[test]
fn volume_and_mute_messages_update_shared_settings_without_audio_hardware() {
    let mut app = playback_app();
    app.world_mut()
        .write_message(SetBgmVolumeEvent { volume: 0.25 });
    app.world_mut()
        .write_message(SetSfxVolumeEvent { volume: 2.0 });
    app.world_mut()
        .write_message(SetAmbienceVolumeEvent { volume: -1.0 });
    app.world_mut().write_message(MuteBgmEvent { muted: true });
    app.world_mut().write_message(MuteSfxEvent { muted: true });
    app.world_mut()
        .write_message(MuteAmbienceEvent { muted: true });
    app.update();
    let settings = app.world().resource::<AudioSettings>();
    assert_eq!(
        (
            settings.bgm_volume,
            settings.sfx_volume,
            settings.ambience_volume
        ),
        (0.25, 1.0, 0.0)
    );
    assert_eq!(
        (
            settings.effective_bgm_volume(),
            settings.effective_sfx_volume(),
            settings.effective_ambience_volume()
        ),
        (0.0, 0.0, 0.0)
    );
}

#[test]
fn bgm_requests_deduplicate_and_crossfade_state_tracks_the_outgoing_handle() {
    let mut app = playback_app();
    app.world_mut()
        .write_message(PlayBgmEvent::new("ro://bgm/01.mp3".into()));
    app.update();
    let first = app
        .world()
        .resource::<BgmManager>()
        .active_instance
        .clone()
        .unwrap();
    assert_eq!(
        app.world().resource::<Audio>().state(&first),
        PlaybackState::Queued
    );
    app.world_mut()
        .write_message(PlayBgmEvent::new("ro://bgm/01.mp3".into()));
    app.update();
    assert_eq!(
        app.world()
            .resource::<BgmManager>()
            .active_instance
            .as_ref(),
        Some(&first)
    );
    app.world_mut()
        .write_message(PlayBgmEvent::new("ro://bgm/02.mp3".into()));
    app.update();
    let second = app
        .world()
        .resource::<BgmManager>()
        .active_instance
        .clone()
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(
        app.world()
            .resource::<BgmManager>()
            .current_track_path
            .as_deref(),
        Some("ro://bgm/02.mp3")
    );
    assert_eq!(
        app.world().resource::<Audio>().state(&second),
        PlaybackState::Queued
    );
    let mut manager = app.world_mut().resource_mut::<BgmManager>();
    let outgoing = manager.take_active_for_fadeout().unwrap();
    manager.add_fading_out(outgoing.clone());
    assert_eq!(outgoing, second);
    assert_eq!(manager.fading_out_instances, vec![second]);
    assert!(manager.current_track_path.is_none());
    manager.cleanup_stopped(&Assets::default());
    assert!(manager.fading_out_instances.is_empty());
}

#[test]
fn a_caller_can_choose_any_entity_as_listener() {
    let mut world = World::new();
    let listener = world.spawn(ContentAudioListener).id();
    assert_eq!(
        world
            .query_filtered::<Entity, With<ContentAudioListener>>()
            .single(&world)
            .unwrap(),
        listener
    );
    let settings = AudioSettings {
        sfx_muted: true,
        ..default()
    };
    assert_eq!(settings.effective_sfx_volume(), 0.0);
}
