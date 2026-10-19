use super::resources::{AmbienceChannel, SfxChannel};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_kira_audio::prelude::{AudioApp, SpatialAudioPlugin};
use bevy_kira_audio::{AudioPlugin as KiraAudioPlugin, DefaultSpatialRadius};

/// A roughly 30-cell, zoom-independent falloff measured from the caller's receiver.
const SFX_SPATIAL_RADIUS_WORLD: f32 = 150.0;

/// Optional audio playback capability. Requires RuntimeCorePlugin's messages.
#[derive(AutoPlugin)]
pub struct ContentAudioPlugin;

impl Plugin for ContentAudioPlugin {
    #[auto_plugin]
    fn build(&self, app: &mut App) {
        app.add_plugins((KiraAudioPlugin, SpatialAudioPlugin))
            .add_audio_channel::<SfxChannel>()
            .add_audio_channel::<AmbienceChannel>()
            .insert_resource(DefaultSpatialRadius {
                radius: SFX_SPATIAL_RADIUS_WORLD,
            });
    }
}
