use crate::domain::audio::resources::{AmbienceChannel, SfxChannel};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_kira_audio::prelude::{AudioApp, SpatialAudioPlugin};
use bevy_kira_audio::{AudioPlugin as KiraAudioPlugin, DefaultSpatialRadius};

/// Spatial falloff radius in world units: volume is full at the receiver (the
/// local player) and fades to silence at this distance. A map cell is 5 world
/// units, so this ~30-cell radius keeps mobs across the visible area audible
/// while distant ones fade out. Zoom-independent (measured from the character,
/// not the camera). Starting point; tune by ear.
const SFX_SPATIAL_RADIUS_WORLD: f32 = 150.0;

/// Audio Plugin
///
/// Wires bevy_kira_audio (BGM + spatial SFX channels); domain audio systems
/// register themselves via `auto_*` attributes in `domain/audio/*`.
#[derive(AutoPlugin)]
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    #[auto_plugin]
    fn build(&self, app: &mut App) {
        app.add_plugins(KiraAudioPlugin)
            .add_plugins(SpatialAudioPlugin)
            .add_audio_channel::<SfxChannel>()
            .add_audio_channel::<AmbienceChannel>()
            .insert_resource(DefaultSpatialRadius {
                radius: SFX_SPATIAL_RADIUS_WORLD,
            });
    }
}
