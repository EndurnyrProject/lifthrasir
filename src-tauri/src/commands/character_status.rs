use crate::bridge::{AppBridge, CharacterStatusPayload};
use tauri::State;

#[tauri::command]
pub async fn get_character_status(
    app_bridge: State<'_, AppBridge>,
) -> Result<CharacterStatusPayload, String> {
    let receiver = app_bridge.send_get_character_status();

    match tokio::time::timeout(std::time::Duration::from_secs(10), receiver).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("Failed to receive character status response: channel closed unexpectedly".into()),
        Err(_) => Err("Character status request timed out after 10 seconds - game engine may not be responding".into()),
    }
}
