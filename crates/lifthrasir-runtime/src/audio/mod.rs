pub mod controlled_sfx;
pub mod events;
pub mod map_sounds;
pub mod playback;
mod plugin;
pub mod resources;
pub use plugin::ContentAudioPlugin;
pub use resources::{AmbienceChannel, AudioSettings, BgmManager, SfxChannel};

/// Caller-selected listener, attached alongside Kira's spatial receiver.
#[derive(bevy::prelude::Component, Debug, Default)]
pub struct ContentAudioListener;
