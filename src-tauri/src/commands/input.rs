use crate::bridge::AppBridge;
use tauri::State;

/// Forward keyboard input from JavaScript to Bevy
/// This allows React UI to send keyboard events to the game engine when not focused on UI elements
#[tauri::command]
pub fn forward_keyboard_input(
    code: String,
    pressed: bool,
    app_bridge: State<'_, AppBridge>,
) -> Result<(), String> {
    app_bridge.forward_keyboard_input(code, pressed)
}

/// Forward mouse position from JavaScript to Bevy
/// This allows React UI to send mouse cursor position to the game engine for debug visualization
#[tauri::command]
pub fn forward_mouse_position(
    x: f32,
    y: f32,
    app_bridge: State<'_, AppBridge>,
) -> Result<(), String> {
    app_bridge.forward_mouse_position(x, y)
}
