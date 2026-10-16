use super::{
    events::PlayBgmEvent,
    resources::{BgmManager, BgmNameTable},
};
use crate::infrastructure::assets::BgmNameTableAsset;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
pub use lifthrasir_runtime::audio::playback::{
    apply_initial_sfx_volume, cleanup_fading_bgm, handle_ambience_mute_change,
    handle_ambience_volume_change, handle_bgm_change, handle_bgm_stop, handle_mute_change,
    handle_sfx_mute_change, handle_sfx_volume_change, handle_volume_change, play_mob_sfx,
    play_skill_sfx,
};

/// System to load the BGM name table from mp3nametable.txt
/// Runs once at startup to load the table asset
#[auto_add_system(
    plugin = crate::domain::audio::plugin::AudioPlugin,
    schedule = Startup
)]
pub fn load_bgm_name_table(
    mut bgm_name_table: ResMut<BgmNameTable>,
    asset_server: Res<AssetServer>,
) {
    if bgm_name_table.table_handle.is_none() {
        debug!("Loading BGM name table from ro://data/mp3nametable.txt");
        let handle: Handle<BgmNameTableAsset> = asset_server.load("ro://data/mp3nametable.txt");
        bgm_name_table.table_handle = Some(handle);
    }
}

/// System to handle map BGM from BGM name table
/// Reads map name from MapRequestLoader and looks up BGM path from mp3nametable.txt
/// Runs every frame and checks if we need to start BGM
#[auto_add_system(
    plugin = crate::domain::audio::plugin::AudioPlugin,
    schedule = Update
)]
pub fn handle_map_bgm(
    mut events: MessageWriter<PlayBgmEvent>,
    query: Query<&crate::domain::world::map::MapData>,
    bgm_name_table: Res<BgmNameTable>,
    bgm_table_assets: Res<Assets<BgmNameTableAsset>>,
    bgm_manager: Res<BgmManager>,
) {
    for map in query.iter() {
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
        let map_name = map.name.to_lowercase();

        if let Some(bgm_path) = bgm_table_asset.table.get(&map_name) {
            let full_bgm_path = format!("ro://{}", bgm_path);

            // Skip if already playing this track
            if bgm_manager.is_playing(&full_bgm_path) {
                continue;
            }

            debug!(
                "Map '{}' has BGM: {} -> {}",
                map.name, bgm_path, full_bgm_path
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

#[auto_add_system(plugin = crate::domain::audio::plugin::AudioPlugin, schedule = Update,
    config(before = lifthrasir_runtime::audio::playback::play_sound_effect))]
pub fn play_sound_effect(
    mut events: MessageReader<net_contract::events::PlaySoundEffect>,
    mut requests: MessageWriter<lifthrasir_runtime::audio::events::PlaySoundEffect>,
) {
    for event in events.read() {
        requests.write(lifthrasir_runtime::audio::events::PlaySoundEffect {
            name: event.name.clone(),
        });
    }
}
