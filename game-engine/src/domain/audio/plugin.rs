use super::{
    events::{MuteBgmEvent, PlayBgmEvent, SetBgmVolumeEvent, StopBgmEvent},
    resources::{AudioSettings, BgmManager, BgmNameTable},
    systems::{
        cleanup_fading_bgm, handle_bgm_change, handle_bgm_stop, handle_map_bgm, handle_mute_change,
        handle_volume_change, load_bgm_name_table,
    },
};
use bevy::prelude::*;

/// Domain plugin for audio systems
/// Manually registers all audio components, events, resources, and systems
pub struct AudioDomainPlugin;

impl Plugin for AudioDomainPlugin {
    fn build(&self, app: &mut App) {
        // Register resources
        app.init_resource::<BgmManager>();
        app.init_resource::<AudioSettings>();
        app.init_resource::<BgmNameTable>();

        // Register events
        app.add_event::<PlayBgmEvent>();
        app.add_event::<StopBgmEvent>();
        app.add_event::<SetBgmVolumeEvent>();
        app.add_event::<MuteBgmEvent>();

        // Register startup system to load BGM name table
        app.add_systems(Startup, load_bgm_name_table);

        // Register systems (all in Update schedule)
        app.add_systems(
            Update,
            (
                handle_bgm_change,
                handle_bgm_stop,
                handle_volume_change,
                handle_mute_change,
                cleanup_fading_bgm,
                handle_map_bgm,
            ),
        );

        debug!("AudioDomainPlugin registered");
    }
}
