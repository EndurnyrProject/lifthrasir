use super::ContentAudioListener;
use crate::world::{EmitterOutput, LifAudioEmitter, SceneOwner};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_kira_audio::prelude::AudioControl;
use bevy_kira_audio::{AudioChannel, AudioInstance, AudioSource, AudioTween, PlaybackState};
use std::time::Duration;

use super::playback::amplitude_to_decibels;
use super::resources::{AmbienceChannel, AudioSettings};

#[derive(Component)]
pub struct MapSound;

pub enum MapSoundState {
    Playing,
    Silent { timer: Timer },
}

#[derive(Component)]
pub struct MapSoundSource {
    pub handle_path: String,
    pub base_volume: f32,
    pub range: f32,
    pub cycle: Duration,
    pub instance: Option<Handle<AudioInstance>>,
    pub state: MapSoundState,
}

pub fn map_sound_path(name: &str) -> String {
    format!("ro://data/wav/{}", name.replace('\\', "/"))
}

/// One scene-owned sound source at its already world-space emitter position.
fn map_sound_bundle(
    translation: Vec3,
    handle_path: String,
    base_volume: f32,
    range: f32,
    cycle: f32,
    owner: Entity,
    emitter: Entity,
) -> impl Bundle {
    (
        MapSound,
        SceneOwner(owner),
        EmitterOutput(emitter),
        MapSoundSource {
            handle_path,
            base_volume,
            range,
            cycle: Duration::from_secs_f32(cycle),
            instance: None,
            state: MapSoundState::Silent {
                timer: Timer::new(Duration::ZERO, TimerMode::Once),
            },
        },
        Transform::from_translation(translation),
    )
}

/// Marks a glb audio-emitter node whose [`MapSound`] has been spawned. Its
/// spawn-once marker dies with the imported scene.
#[derive(Component)]
pub struct GltfMapSoundSpawned;

/// GLB audio emitter instantiation: emitter nodes carry their final
/// world position in `GlobalTransform`, so this must run after transform
/// propagation ([`GltfMapPlugin`] schedules it there).
///
/// Rangeless and fileless sounds never reach a glb -- the converter drops them
/// while emitting nodes, mirroring the guard on the RSW path.
///
/// [`GltfMapPlugin`]: crate::world::MapEmittersPlugin
pub fn spawn_gltf_map_sounds(
    mut commands: Commands,
    emitters: Query<
        (Entity, &LifAudioEmitter, &GlobalTransform, &SceneOwner),
        Without<GltfMapSoundSpawned>,
    >,
) {
    for (entity, emitter, transform, owner) in emitters.iter() {
        let audio = &emitter.0;

        commands.spawn(map_sound_bundle(
            transform.translation(),
            audio.file.clone(),
            audio.volume,
            audio.range,
            audio.cycle,
            owner.0,
            entity,
        ));
        commands.entity(entity).insert(GltfMapSoundSpawned);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SoundCycleInput {
    Playing {
        finished: bool,
    },
    Silent {
        timer_finished: bool,
        in_range: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SoundAction {
    Play,
    BeginSilence,
    None,
}

fn next_sound_action(input: SoundCycleInput) -> SoundAction {
    match input {
        SoundCycleInput::Playing { finished: true } => SoundAction::BeginSilence,
        SoundCycleInput::Playing { finished: false } => SoundAction::None,
        SoundCycleInput::Silent {
            timer_finished: true,
            in_range: true,
        } => SoundAction::Play,
        SoundCycleInput::Silent { .. } => SoundAction::None,
    }
}

fn distance_gain(dist: f32, range: f32) -> f32 {
    if range <= 0.0 {
        return 0.0;
    }
    (1.0 - dist / range).clamp(0.0, 1.0)
}

#[auto_add_system(plugin = crate::audio::ContentAudioPlugin, schedule = Update)]
pub fn drive_map_sound_cycle(
    mut sources: Query<(&mut MapSoundSource, &GlobalTransform)>,
    listener: Query<&GlobalTransform, With<ContentAudioListener>>,
    ambience_channel: Res<AudioChannel<AmbienceChannel>>,
    audio_instances: Res<Assets<AudioInstance>>,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
) {
    let Ok(listener_transform) = listener.single() else {
        return;
    };
    let listener_pos = listener_transform.translation();

    for (mut src, transform) in sources.iter_mut() {
        let in_range = listener_pos.distance(transform.translation()) <= src.range;

        let input = match &mut src.state {
            MapSoundState::Playing => {
                let finished = match &src.instance {
                    Some(handle) => {
                        audio_instances.get(handle).map(|i| i.state())
                            == Some(PlaybackState::Stopped)
                    }
                    None => true,
                };
                SoundCycleInput::Playing { finished }
            }
            MapSoundState::Silent { timer } => {
                timer.tick(time.delta());
                SoundCycleInput::Silent {
                    timer_finished: timer.is_finished(),
                    in_range,
                }
            }
        };

        match next_sound_action(input) {
            SoundAction::Play => {
                let source: Handle<AudioSource> = asset_server.load(&src.handle_path);
                let handle = ambience_channel.play(source).handle();
                src.instance = Some(handle);
                src.state = MapSoundState::Playing;
            }
            SoundAction::BeginSilence => {
                src.state = MapSoundState::Silent {
                    timer: Timer::new(src.cycle, TimerMode::Once),
                };
            }
            SoundAction::None => {}
        }
    }
}

#[derive(Resource)]
#[auto_init_resource(plugin = crate::audio::ContentAudioPlugin)]
pub struct MapSoundUpdateTimer(Timer);

impl Default for MapSoundUpdateTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.1, TimerMode::Repeating))
    }
}

#[auto_add_system(plugin = crate::audio::ContentAudioPlugin, schedule = Update)]
pub fn update_map_sound_volume(
    mut throttle: ResMut<MapSoundUpdateTimer>,
    time: Res<Time>,
    mut sources: Query<(&mut MapSoundSource, &GlobalTransform)>,
    listener: Query<&GlobalTransform, With<ContentAudioListener>>,
    audio_settings: Res<AudioSettings>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
) {
    if !throttle.0.tick(time.delta()).just_finished() {
        return;
    }

    let Ok(listener_transform) = listener.single() else {
        return;
    };
    let listener_pos = listener_transform.translation();

    let tween = AudioTween::linear(Duration::from_secs_f32(0.1));

    for (mut src, transform) in sources.iter_mut() {
        if !matches!(src.state, MapSoundState::Playing) {
            continue;
        }

        let Some(handle) = src.instance.clone() else {
            continue;
        };

        let Some(mut instance) = audio_instances.get_mut(&handle) else {
            continue;
        };

        let dist = listener_pos.distance(transform.translation());

        if dist > src.range {
            instance.stop(tween.clone());
            src.state = MapSoundState::Silent {
                timer: Timer::new(src.cycle, TimerMode::Once),
            };
            continue;
        }

        let gain = distance_gain(dist, src.range);
        let vol = src.base_volume * gain * audio_settings.effective_ambience_volume();
        instance.set_decibels(amplitude_to_decibels(vol), tween.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_cycle_accepts_an_explicit_non_player_listener() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<AudioSource>()
            .init_asset::<AudioInstance>()
            .init_resource::<AudioChannel<AmbienceChannel>>()
            .add_systems(Update, drive_map_sound_cycle);
        let owner = app.world_mut().spawn_empty().id();
        let position = Vec3::new(200.0, 0.0, 0.0);
        let source = app
            .world_mut()
            .spawn((
                map_sound_bundle(
                    position,
                    "ro://sound.wav".into(),
                    0.5,
                    10.0,
                    0.0,
                    owner,
                    owner,
                ),
                GlobalTransform::from_translation(position),
            ))
            .id();
        let listener = app
            .world_mut()
            .spawn((ContentAudioListener, GlobalTransform::default()))
            .id();
        app.update();
        assert!(
            app.world()
                .get::<MapSoundSource>(source)
                .unwrap()
                .instance
                .is_none()
        );
        app.world_mut()
            .entity_mut(listener)
            .insert(GlobalTransform::from_translation(position));
        app.update();
        let handle = app
            .world()
            .get::<MapSoundSource>(source)
            .unwrap()
            .instance
            .as_ref()
            .unwrap();
        assert_eq!(
            app.world()
                .resource::<AudioChannel<AmbienceChannel>>()
                .state(handle),
            PlaybackState::Queued
        );
    }

    #[test]
    fn map_sound_path_normalizes_backslashes_and_prefixes() {
        assert_eq!(map_sound_path("foo\\bar.wav"), "ro://data/wav/foo/bar.wav");
    }

    #[test]
    fn playing_with_finished_instance_begins_silence() {
        assert_eq!(
            next_sound_action(SoundCycleInput::Playing { finished: true }),
            SoundAction::BeginSilence
        );
    }

    #[test]
    fn playing_with_live_instance_does_nothing() {
        assert_eq!(
            next_sound_action(SoundCycleInput::Playing { finished: false }),
            SoundAction::None
        );
    }

    #[test]
    fn silent_finished_in_range_plays() {
        assert_eq!(
            next_sound_action(SoundCycleInput::Silent {
                timer_finished: true,
                in_range: true,
            }),
            SoundAction::Play
        );
    }

    #[test]
    fn silent_finished_out_of_range_stays_silent() {
        assert_eq!(
            next_sound_action(SoundCycleInput::Silent {
                timer_finished: true,
                in_range: false,
            }),
            SoundAction::None
        );
    }

    #[test]
    fn silent_not_finished_does_nothing() {
        assert_eq!(
            next_sound_action(SoundCycleInput::Silent {
                timer_finished: false,
                in_range: true,
            }),
            SoundAction::None
        );
    }

    #[test]
    fn distance_gain_is_full_at_zero_distance() {
        assert_eq!(distance_gain(0.0, 50.0), 1.0);
    }

    #[test]
    fn distance_gain_is_half_at_mid_range() {
        assert!((distance_gain(25.0, 50.0) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn distance_gain_is_zero_at_and_beyond_range() {
        assert_eq!(distance_gain(50.0, 50.0), 0.0);
        assert_eq!(distance_gain(75.0, 50.0), 0.0);
    }

    #[test]
    fn distance_gain_guards_non_positive_range() {
        assert_eq!(distance_gain(0.0, 0.0), 0.0);
        assert_eq!(distance_gain(5.0, -10.0), 0.0);
    }

    #[test]
    fn zero_cycle_replays_immediately() {
        let mut timer = Timer::new(Duration::ZERO, TimerMode::Once);
        timer.tick(Duration::from_millis(1));
        assert!(timer.is_finished());
        assert_eq!(
            next_sound_action(SoundCycleInput::Silent {
                timer_finished: timer.is_finished(),
                in_range: true,
            }),
            SoundAction::Play
        );
    }
}
