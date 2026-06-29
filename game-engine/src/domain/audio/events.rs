use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// Event to request playing a BGM track with crossfading
#[derive(Message, Debug, Clone, Reflect)]
#[reflect(Debug)]
#[auto_add_message(plugin = crate::app::audio_plugin::AudioPlugin)]
pub struct PlayBgmEvent {
    /// Path to the BGM file (e.g., "ro://data/bgm/01.mp3")
    pub path: String,
    /// Fade-in duration in seconds (default: 2.0)
    pub fade_in_duration: f32,
    /// Fade-out duration for the previous track in seconds (default: 2.0)
    pub fade_out_duration: f32,
}

impl PlayBgmEvent {
    pub fn new(path: String) -> Self {
        Self {
            path,
            fade_in_duration: 2.0,
            fade_out_duration: 2.0,
        }
    }
}

/// Event to request stopping the current BGM
#[derive(Message, Debug, Clone, Copy, Reflect)]
#[reflect(Debug)]
#[auto_add_message(plugin = crate::app::audio_plugin::AudioPlugin)]
pub struct StopBgmEvent {
    pub fade_out_duration: f32,
}

impl Default for StopBgmEvent {
    fn default() -> Self {
        Self {
            fade_out_duration: 2.0,
        }
    }
}

/// Event to change the BGM volume
#[derive(Message, Debug, Clone, Copy, Reflect)]
#[reflect(Debug)]
#[auto_add_message(plugin = crate::app::audio_plugin::AudioPlugin)]
pub struct SetBgmVolumeEvent {
    /// Volume level (0.0 to 1.0)
    pub volume: f32,
}

/// Event to mute or unmute the BGM
#[derive(Message, Debug, Clone, Copy, Reflect)]
#[reflect(Debug)]
#[auto_add_message(plugin = crate::app::audio_plugin::AudioPlugin)]
pub struct MuteBgmEvent {
    /// Whether to mute (true) or unmute (false)
    pub muted: bool,
}

/// Event requesting a mob sound effect be played, anchored to a spatial emitter entity.
#[derive(Message, Debug, Clone, Reflect)]
#[reflect(Debug)]
#[auto_add_message(plugin = crate::app::audio_plugin::AudioPlugin)]
pub struct PlayMobSfx {
    /// Entity carrying the `SpatialAudioEmitter` (the mob root).
    pub emitter: Entity,
    /// Raw decoded sound filename from the ACT (e.g. "포링.wav" or "monster\\xxx.wav").
    pub sound: String,
}

/// Event requesting a skill sound effect, anchored to a spatial emitter entity.
/// Unlike [`PlayMobSfx`], the player handler attaches a `SpatialAudioEmitter` to
/// the emitter if it lacks one (skill effect entities and player casters are not
/// guaranteed to carry one), so it works for any anchor entity.
#[derive(Message, Debug, Clone, Reflect)]
#[reflect(Debug)]
#[auto_add_message(plugin = crate::app::audio_plugin::AudioPlugin)]
pub struct PlaySkillSfx {
    /// Entity the sound is anchored to (the effect's anchor unit or cell entity).
    pub emitter: Entity,
    /// Sound path relative to `data/wav/` (e.g. "effect/ef_firewall.wav", or
    /// "_heal_effect.wav" for files at the wav root).
    pub sound: String,
}

/// Event to change the SFX volume.
#[derive(Message, Debug, Clone, Copy, Reflect)]
#[reflect(Debug)]
#[auto_add_message(plugin = crate::app::audio_plugin::AudioPlugin)]
pub struct SetSfxVolumeEvent {
    /// Volume level (0.0 to 1.0)
    pub volume: f32,
}

/// Event to mute or unmute SFX.
#[derive(Message, Debug, Clone, Copy, Reflect)]
#[reflect(Debug)]
#[auto_add_message(plugin = crate::app::audio_plugin::AudioPlugin)]
pub struct MuteSfxEvent {
    pub muted: bool,
}

/// Event to change the ambience volume.
#[derive(Message, Debug, Clone, Copy, Reflect)]
#[reflect(Debug)]
#[auto_add_message(plugin = crate::app::audio_plugin::AudioPlugin)]
pub struct SetAmbienceVolumeEvent {
    /// Volume level (0.0 to 1.0)
    pub volume: f32,
}

/// Event to mute or unmute ambience.
#[derive(Message, Debug, Clone, Copy, Reflect)]
#[reflect(Debug)]
#[auto_add_message(plugin = crate::app::audio_plugin::AudioPlugin)]
pub struct MuteAmbienceEvent {
    pub muted: bool,
}
