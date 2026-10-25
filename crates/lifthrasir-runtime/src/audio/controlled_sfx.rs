//! Cancellable skill voices, isolated from other sounds on the shared SFX channel.

use super::events::ControlSkillSfx;
use super::playback::sfx_path;
use super::resources::SfxChannel;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_kira_audio::{
    AudioChannel, AudioControl, AudioInstance, AudioSource, AudioTween, PlaybackState,
    SpatialAudioEmitter,
};
use std::collections::HashMap;

struct ControlledSound {
    source: Handle<AudioSource>,
    instance: Option<Handle<AudioInstance>>,
}

/// One cancellable voice per emitter and instance stops awaiting Kira playback.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::audio::ContentAudioPlugin)]
pub struct ControlledSkillSounds {
    active: HashMap<Entity, ControlledSound>,
    stopping: Vec<Handle<AudioInstance>>,
}

/// Apply requests in order so a cast and its cancellation in one frame cannot
/// restart the sound. Unloaded sources stay here rather than in Kira's play queue.
#[auto_add_system(
    plugin = crate::audio::ContentAudioPlugin,
    schedule = Update,
    config(before = play_controlled_skill_sfx)
)]
pub fn control_skill_sfx(
    mut events: MessageReader<ControlSkillSfx>,
    asset_server: Res<AssetServer>,
    mut sounds: ResMut<ControlledSkillSounds>,
) {
    for event in events.read() {
        let emitter = match event {
            ControlSkillSfx::Play { emitter, .. } | ControlSkillSfx::Stop { emitter } => *emitter,
        };
        if let Some(previous) = sounds.active.remove(&emitter)
            && let Some(instance) = previous.instance
        {
            sounds.stopping.push(instance);
        }
        if let ControlSkillSfx::Play { sound, .. } = event {
            sounds.active.insert(
                emitter,
                ControlledSound {
                    source: asset_server.load(sfx_path(sound)),
                    instance: None,
                },
            );
        }
    }
}

/// Start loaded voices, retire finished or orphaned ones, and stop only the
/// requested instances. A handle can remain queued until Kira's PostUpdate pass.
#[auto_add_system(
    plugin = crate::audio::ContentAudioPlugin,
    schedule = Update,
    config(after = crate::audio::playback::play_skill_sfx)
)]
pub fn play_controlled_skill_sfx(
    mut commands: Commands,
    mut sounds: ResMut<ControlledSkillSounds>,
    sources: Res<Assets<AudioSource>>,
    mut instances: ResMut<Assets<AudioInstance>>,
    channel: Res<AudioChannel<SfxChannel>>,
    mut emitters: Query<Option<&mut SpatialAudioEmitter>>,
) {
    let ControlledSkillSounds { active, stopping } = &mut *sounds;
    active.retain(|entity, sound| {
        let Ok(emitter) = emitters.get_mut(*entity) else {
            if let Some(instance) = sound.instance.take() {
                stopping.push(instance);
            }
            return false;
        };
        if let Some(instance) = &sound.instance {
            return !matches!(channel.state(instance), PlaybackState::Stopped);
        }
        if !sources.contains(&sound.source) {
            return true;
        }
        let instance = channel.play(sound.source.clone()).handle();
        match emitter {
            Some(mut emitter) => emitter.instances.push(instance.clone()),
            None => {
                commands.entity(*entity).insert(SpatialAudioEmitter {
                    instances: vec![instance.clone()],
                });
            }
        }
        sound.instance = Some(instance);
        true
    });
    stopping.retain(|handle| {
        if let Some(mut instance) = instances.get_mut(handle) {
            instance.stop(AudioTween::default());
            false
        } else {
            matches!(channel.state(handle), PlaybackState::Queued)
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_kira_audio::prelude::{Frame, StaticSoundData};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<AudioSource>()
            .init_asset::<AudioInstance>()
            .init_resource::<AudioChannel<SfxChannel>>()
            .init_resource::<ControlledSkillSounds>()
            .add_message::<ControlSkillSfx>()
            .add_systems(
                Update,
                (control_skill_sfx, play_controlled_skill_sfx).chain(),
            );
        app
    }

    #[test]
    fn cancel_unloaded_voice_preserves_other_emitters() {
        let mut app = test_app();
        let caster = app.world_mut().spawn_empty().id();
        let other = app.world_mut().spawn_empty().id();
        for emitter in [caster, other] {
            app.world_mut().write_message(ControlSkillSfx::Play {
                emitter,
                sound: "effect/song.wav".into(),
            });
        }
        app.update();
        assert!(
            app.world()
                .resource::<ControlledSkillSounds>()
                .active
                .values()
                .all(|sound| sound.instance.is_none()),
            "unloaded sounds must not enter Kira's queue"
        );
        app.world_mut()
            .write_message(ControlSkillSfx::Stop { emitter: caster });
        app.update();
        let sounds = app.world().resource::<ControlledSkillSounds>();
        assert_eq!(sounds.active.len(), 1);
        assert!(sounds.active.contains_key(&other));
        assert!(
            sounds.stopping.is_empty(),
            "the canceled source never played"
        );
    }

    #[test]
    fn canceled_queued_voices_wait_for_backend_without_stopping_other_sfx() {
        let mut app = test_app();
        let caster = app.world_mut().spawn_empty().id();
        let other = app.world_mut().spawn_empty().id();
        for emitter in [caster, other] {
            app.world_mut().write_message(ControlSkillSfx::Play {
                emitter,
                sound: "effect/song.wav".into(),
            });
        }
        app.update();
        let source = app.world().resource::<ControlledSkillSounds>().active[&caster]
            .source
            .clone();
        app.world_mut()
            .resource_mut::<Assets<AudioSource>>()
            .insert(
                source.id(),
                AudioSource {
                    sound: StaticSoundData {
                        sample_rate: 44_100,
                        frames: vec![Frame::ZERO; 16].into(),
                        settings: Default::default(),
                        slice: None,
                    },
                },
            )
            .unwrap();
        app.update();
        let sounds = app.world().resource::<ControlledSkillSounds>();
        let canceled = sounds.active[&caster].instance.clone().unwrap();
        let preserved = sounds.active[&other].instance.clone().unwrap();
        let ordinary = app
            .world()
            .resource::<AudioChannel<SfxChannel>>()
            .play(source)
            .handle();
        app.world_mut()
            .write_message(ControlSkillSfx::Stop { emitter: caster });
        app.update();
        let sounds = app.world().resource::<ControlledSkillSounds>();
        assert_eq!(sounds.stopping, vec![canceled.clone()]);
        assert_eq!(sounds.active[&other].instance.as_ref(), Some(&preserved));

        app.world_mut().despawn(other);
        app.update();
        let sounds = app.world().resource::<ControlledSkillSounds>();
        assert!(
            sounds.active.is_empty(),
            "despawned emitters stop their voices"
        );
        assert_eq!(sounds.stopping, vec![canceled, preserved]);
        assert!(!sounds.stopping.contains(&ordinary));
        assert_eq!(
            app.world()
                .resource::<AudioChannel<SfxChannel>>()
                .state(&ordinary),
            PlaybackState::Queued,
            "ordinary skill sounds are not canceled"
        );
    }

    #[test]
    fn same_frame_cast_cancel_and_recast_preserve_request_order() {
        let mut app = test_app();
        let caster = app.world_mut().spawn_empty().id();
        for request in [
            ControlSkillSfx::Play {
                emitter: caster,
                sound: "effect/old.wav".into(),
            },
            ControlSkillSfx::Stop { emitter: caster },
            ControlSkillSfx::Play {
                emitter: caster,
                sound: "effect/new.wav".into(),
            },
        ] {
            app.world_mut().write_message(request);
        }
        app.update();
        let sounds = app.world().resource::<ControlledSkillSounds>();
        assert_eq!(sounds.active.len(), 1);
        let path = app
            .world()
            .resource::<AssetServer>()
            .get_path(sounds.active[&caster].source.id())
            .unwrap();
        assert_eq!(path.to_string(), "ro://data/wav/effect/new.wav");
        assert!(
            sounds.stopping.is_empty(),
            "the old voice never reached Kira"
        );
    }
}
