use bevy_auto_plugin::prelude::{auto_plugin, AutoPlugin};

/// Input Plugin
///
/// Handles all input including:
/// - Cursor position
/// - Mouse clicks
/// - Terrain cursor visualization
/// - Cursor state management (default, attack, impossible, etc.)
/// - Terrain raycasting cache
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct InputPlugin;
