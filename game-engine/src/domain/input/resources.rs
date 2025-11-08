use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_init_resource;

/// Resource to store cursor position forwarded from Tauri UI
/// This allows the game to know where the mouse cursor is even though
/// Tauri's webview overlay captures the actual mouse events
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::app::input_plugin::InputPlugin)]
pub struct ForwardedCursorPosition {
    pub position: Option<Vec2>,
}

/// Resource to store mouse click position forwarded from Tauri UI
/// This is used to handle terrain clicks for player movement
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::app::input_plugin::InputPlugin)]
pub struct ForwardedMouseClick {
    pub position: Option<Vec2>,
}
