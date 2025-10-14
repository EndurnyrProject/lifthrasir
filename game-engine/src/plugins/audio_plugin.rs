use crate::domain::audio::AudioDomainPlugin;
use bevy::prelude::*;
use bevy_kira_audio::AudioPlugin as KiraAudioPlugin;

/// Main audio plugin that integrates bevy_kira_audio and domain audio logic
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app
            // Add bevy_kira_audio plugin
            .add_plugins(KiraAudioPlugin)
            // Add domain audio plugin (registers components, events, resources, systems)
            .add_plugins(AudioDomainPlugin);

        info!("AudioPlugin initialized with BGM system");
    }
}
