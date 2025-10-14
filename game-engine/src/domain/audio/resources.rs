use crate::infrastructure::assets::BgmNameTableAsset;
use bevy::prelude::*;
use bevy_kira_audio::AudioInstance;

/// Resource that manages the BGM playback state
/// Tracks active and fading out audio instances for crossfading
#[derive(Resource, Debug, Default, Reflect)]
#[reflect(Resource, Debug)]
pub struct BgmManager {
    /// The currently active BGM instance (playing or fading in)
    #[reflect(ignore)]
    pub active_instance: Option<Handle<AudioInstance>>,
    /// BGM instances that are fading out (will be cleaned up when stopped)
    #[reflect(ignore)]
    pub fading_out_instances: Vec<Handle<AudioInstance>>,
    /// Path to the currently playing track (for duplicate prevention)
    pub current_track_path: Option<String>,
}

impl BgmManager {
    /// Check if a specific track is already playing
    pub fn is_playing(&self, path: &str) -> bool {
        self.current_track_path.as_deref() == Some(path)
    }

    /// Set the active track
    pub fn set_active(&mut self, instance: Handle<AudioInstance>, path: String) {
        self.active_instance = Some(instance);
        self.current_track_path = Some(path);
    }

    /// Take the active instance and prepare it for fade-out
    pub fn take_active_for_fadeout(&mut self) -> Option<Handle<AudioInstance>> {
        self.current_track_path = None;
        self.active_instance.take()
    }

    /// Add an instance to the fading out list
    pub fn add_fading_out(&mut self, instance: Handle<AudioInstance>) {
        self.fading_out_instances.push(instance);
    }

    /// Remove stopped instances from the fading out list
    pub fn cleanup_stopped(&mut self, instances: &Assets<AudioInstance>) {
        self.fading_out_instances.retain(|handle| {
            instances
                .get(handle)
                .map(|instance| instance.state() != bevy_kira_audio::PlaybackState::Stopped)
                .unwrap_or(false)
        });
    }
}

/// Resource that stores audio settings (volume, mute state)
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource, Debug)]
pub struct AudioSettings {
    /// BGM volume (0.0 to 1.0)
    pub bgm_volume: f32,
    /// Whether BGM is muted
    pub bgm_muted: bool,
    /// SFX volume (0.0 to 1.0) - for future use
    pub sfx_volume: f32,
    /// Whether SFX is muted - for future use
    pub sfx_muted: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            bgm_volume: 0.7,
            bgm_muted: false,
            sfx_volume: 0.8,
            sfx_muted: false,
        }
    }
}

impl AudioSettings {
    /// Get the effective BGM volume (considering mute state)
    pub fn effective_bgm_volume(&self) -> f32 {
        if self.bgm_muted {
            0.0
        } else {
            self.bgm_volume
        }
    }
}

/// Resource that holds the BGM name table asset handle
/// This table maps map names to BGM file paths from mp3nametable.txt
#[derive(Resource, Debug, Default)]
pub struct BgmNameTable {
    pub table_handle: Option<Handle<BgmNameTableAsset>>,
}
