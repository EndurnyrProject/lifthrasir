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
pub struct AudioDomainPlugin;

impl Plugin for AudioDomainPlugin {
    fn build(&self, app: &mut App) {
        // Register resources
        app.init_resource::<BgmManager>();
        app.init_resource::<AudioSettings>();
        app.init_resource::<BgmNameTable>();

        // Register events
        app.add_message::<PlayBgmEvent>();
        app.add_message::<StopBgmEvent>();
        app.add_message::<SetBgmVolumeEvent>();
        app.add_message::<MuteBgmEvent>();

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
