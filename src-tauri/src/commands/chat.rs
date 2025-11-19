use crate::bridge::AppBridge;
use tauri::State;

/// Send a chat message from the UI to the game engine
#[tauri::command]
pub fn send_chat_message(
    message: String,
    app_bridge: State<'_, AppBridge>,
) -> Result<(), String> {
    app_bridge.forward_chat_message(message)
}
