use super::{
    events::{MuteBgmEvent, PlayBgmEvent, SetBgmVolumeEvent, StopBgmEvent},
    resources::{AudioSettings, BgmManager, BgmNameTable},
};
use crate::infrastructure::assets::BgmNameTableAsset;
use bevy::prelude::*;
use bevy_kira_audio::{Audio, AudioControl, AudioInstance, AudioSource, AudioTween};

/// System to handle BGM change requests with crossfading
/// Listens for PlayBgmEvent and manages track transitions
pub fn handle_bgm_change(
    mut events: EventReader<PlayBgmEvent>,
    mut bgm_manager: ResMut<BgmManager>,
    audio_settings: Res<AudioSettings>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
) {
    for event in events.read() {
        // Skip if already playing the same track
        if bgm_manager.is_playing(&event.path) {
            debug!("BGM track '{}' is already playing, skipping", event.path);
            continue;
        }

        debug!(
            "Starting BGM track '{}' (fade_in: {}s, fade_out: {}s)",
            event.path, event.fade_in_duration, event.fade_out_duration
        );

        // Fade out current track if one is playing
        if let Some(active_handle) = bgm_manager.take_active_for_fadeout() {
            if let Some(active_instance) = audio_instances.get_mut(&active_handle) {
                debug!(
                    "Fading out previous BGM track over {}s",
                    event.fade_out_duration
                );
                active_instance.stop(AudioTween::linear(std::time::Duration::from_secs_f32(
                    event.fade_out_duration,
                )));
                bgm_manager.add_fading_out(active_handle);
            }
        }

        // Load and play new track
        let audio_source: Handle<AudioSource> = asset_server.load(&event.path);

        // Play with fade-in and volume settings
        let effective_volume = audio_settings.effective_bgm_volume() as f64;
        let instance_handle = audio
            .play(audio_source)
            .looped()
            .with_volume(0.0) // Start at 0 volume for fade-in
            .handle();

        // Apply fade-in after starting
        if let Some(instance) = audio_instances.get_mut(&instance_handle) {
            instance.set_volume(
                effective_volume,
                AudioTween::linear(std::time::Duration::from_secs_f32(event.fade_in_duration)),
            );
        }

        // Set as active track
        bgm_manager.set_active(instance_handle, event.path.clone());
    }
}

/// System to handle BGM stop requests
/// Fades out and stops the current track
pub fn handle_bgm_stop(
    mut events: EventReader<StopBgmEvent>,
    mut bgm_manager: ResMut<BgmManager>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
) {
    for event in events.read() {
        if let Some(active_handle) = bgm_manager.take_active_for_fadeout() {
            if let Some(active_instance) = audio_instances.get_mut(&active_handle) {
                debug!("Stopping BGM with {}s fade-out", event.fade_out_duration);
                active_instance.stop(AudioTween::linear(std::time::Duration::from_secs_f32(
                    event.fade_out_duration,
                )));
                bgm_manager.add_fading_out(active_handle);
            }
        } else {
            debug!("StopBgmEvent received but no BGM is playing");
        }
    }
}

/// System to handle BGM volume changes
/// Applies volume immediately to active and fading tracks
pub fn handle_volume_change(
    mut events: EventReader<SetBgmVolumeEvent>,
    mut audio_settings: ResMut<AudioSettings>,
    bgm_manager: Res<BgmManager>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
) {
    for event in events.read() {
        let clamped_volume = event.volume.clamp(0.0, 1.0);
        debug!("Setting BGM volume to {}", clamped_volume);
        audio_settings.bgm_volume = clamped_volume;

        let effective_volume = audio_settings.effective_bgm_volume() as f64;

        // Apply to active track
        if let Some(active_handle) = &bgm_manager.active_instance {
            if let Some(instance) = audio_instances.get_mut(active_handle) {
                instance.set_volume(effective_volume, AudioTween::default());
            }
        }

        // Apply to fading tracks (they should fade to the new volume level)
        for fading_handle in &bgm_manager.fading_out_instances {
            if let Some(_instance) = audio_instances.get_mut(fading_handle) {
                // Note: Fading tracks are already stopping, so we don't change their volume
                // as it would interfere with the fade-out. This is intentional.
            }
        }
    }
}

/// System to handle BGM mute/unmute requests
/// Instantly mutes or unmutes all BGM
pub fn handle_mute_change(
    mut events: EventReader<MuteBgmEvent>,
    mut audio_settings: ResMut<AudioSettings>,
    bgm_manager: Res<BgmManager>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
) {
    for event in events.read() {
        debug!("Setting BGM muted to {}", event.muted);
        audio_settings.bgm_muted = event.muted;

        let effective_volume = audio_settings.effective_bgm_volume() as f64;

        // Apply to active track
        if let Some(active_handle) = &bgm_manager.active_instance {
            if let Some(instance) = audio_instances.get_mut(active_handle) {
                instance.set_volume(effective_volume, AudioTween::default());
            }
        }

        // Mute/unmute fading tracks as well
        for fading_handle in &bgm_manager.fading_out_instances {
            if let Some(instance) = audio_instances.get_mut(fading_handle) {
                instance.set_volume(effective_volume, AudioTween::default());
            }
        }
    }
}

/// System to cleanup stopped fading-out BGM instances
/// Runs every frame to remove completed fade-outs
pub fn cleanup_fading_bgm(
    mut bgm_manager: ResMut<BgmManager>,
    audio_instances: Res<Assets<AudioInstance>>,
) {
    let initial_count = bgm_manager.fading_out_instances.len();
    bgm_manager.cleanup_stopped(&audio_instances);
    let removed_count = initial_count - bgm_manager.fading_out_instances.len();

    if removed_count > 0 {
        debug!("Cleaned up {} stopped BGM instances", removed_count);
    }
}

/// System to load the BGM name table from mp3nametable.txt
/// Runs once at startup to load the table asset
pub fn load_bgm_name_table(
    mut bgm_name_table: ResMut<BgmNameTable>,
    asset_server: Res<AssetServer>,
) {
    if bgm_name_table.table_handle.is_none() {
        info!("Loading BGM name table from ro://data/mp3nametable.txt");
        let handle: Handle<BgmNameTableAsset> = asset_server.load("ro://data/mp3nametable.txt");
        bgm_name_table.table_handle = Some(handle);
    }
}

/// System to handle map BGM from BGM name table
/// Reads map name from MapRequestLoader and looks up BGM path from mp3nametable.txt
/// Runs every frame and checks if we need to start BGM
pub fn handle_map_bgm(
    mut events: EventWriter<PlayBgmEvent>,
    query: Query<(
        &crate::domain::world::components::MapLoader,
        &crate::domain::world::map_loader::MapRequestLoader,
    )>,
    bgm_name_table: Res<BgmNameTable>,
    bgm_table_assets: Res<Assets<BgmNameTableAsset>>,
    bgm_manager: Res<BgmManager>,
) {
    for (_map_loader, map_request) in query.iter() {
        // Skip if map is not loaded yet
        if !map_request.loaded {
            continue;
        }

        // Get the BGM name table asset
        let Some(table_handle) = &bgm_name_table.table_handle else {
            debug!("BGM name table not loaded yet");
            continue;
        };

        let Some(bgm_table_asset) = bgm_table_assets.get(table_handle) else {
            debug!("BGM name table asset not ready");
            continue;
        };

        // Normalize map name for BGM table lookup
        // Strip .gat extension and lowercase to match table keys
        // Table has keys like "aldebaran" (from "aldebaran.rsw")
        let map_name = map_request.map_name.trim_end_matches(".gat").to_lowercase();

        if let Some(bgm_path) = bgm_table_asset.table.get(&map_name) {
            let full_bgm_path = format!("ro://data/{}", bgm_path);

            // Skip if already playing this track
            if bgm_manager.is_playing(&full_bgm_path) {
                continue;
            }

            debug!(
                "Map '{}' has BGM: {} -> {}",
                map_request.map_name, bgm_path, full_bgm_path
            );
            events.write(PlayBgmEvent::new(full_bgm_path));
        } else {
            debug!(
                "No BGM entry found in mp3nametable.txt for map '{}'",
                map_name
            );
        }
    }
}
