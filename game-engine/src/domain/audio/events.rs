use bevy::prelude::*;

/// Event to request playing a BGM track with crossfading
#[derive(Event, Debug, Clone, Reflect)]
#[reflect(Debug)]
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

    pub fn with_fade_durations(path: String, fade_in: f32, fade_out: f32) -> Self {
        Self {
            path,
            fade_in_duration: fade_in,
            fade_out_duration: fade_out,
        }
    }
}

/// Event to request stopping the current BGM
#[derive(Event, Debug, Clone, Copy, Reflect)]
#[reflect(Debug)]
pub struct StopBgmEvent {
    /// Fade-out duration in seconds (default: 2.0)
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
#[derive(Event, Debug, Clone, Copy, Reflect)]
#[reflect(Debug)]
pub struct SetBgmVolumeEvent {
    /// Volume level (0.0 to 1.0)
    pub volume: f32,
}

/// Event to mute or unmute the BGM
#[derive(Event, Debug, Clone, Copy, Reflect)]
#[reflect(Debug)]
pub struct MuteBgmEvent {
    /// Whether to mute (true) or unmute (false)
    pub muted: bool,
}
