use bevy_auto_plugin::modes::global::prelude::{auto_plugin, AutoPlugin};

/// Audio Plugin
///
/// Handles all audio functionality including:
/// - BGM playback with crossfading
/// - Volume and mute controls
/// - Map-based BGM triggering
/// - BGM name table loading
///
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct AudioPlugin;
