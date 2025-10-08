use bevy::prelude::*;

/// Resource to store cursor position forwarded from Tauri UI
/// This allows the game to know where the mouse cursor is even though
/// Tauri's webview overlay captures the actual mouse events
#[derive(Resource, Default)]
pub struct ForwardedCursorPosition {
    pub position: Option<Vec2>,
}
