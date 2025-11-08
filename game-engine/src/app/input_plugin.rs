use bevy_auto_plugin::modes::global::prelude::{auto_plugin, AutoPlugin};

/// Input Plugin
///
/// Handles all input from Tauri UI including:
/// - Cursor position forwarding
/// - Mouse click forwarding
/// - Terrain cursor visualization
/// - Cursor state management (default, attack, impossible, etc.)
/// - Terrain raycasting cache
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct InputPlugin;
