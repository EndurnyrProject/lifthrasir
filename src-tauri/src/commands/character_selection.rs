use crate::bridge::AppBridge;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
pub struct CreateCharacterRequest {
    pub name: String,
    pub slot: u8,
    pub hair_style: u16,
    pub hair_color: u16,
    pub sex: u8, // 0 = Female, 1 = Male
}

/// Get character list command - fetches all characters from Bevy
#[tauri::command]
pub async fn get_character_list(
    app_bridge: State<'_, AppBridge>,
) -> Result<serde_json::Value, String> {
    let characters = app_bridge.get_character_list().await?;
    Ok(serde_json::json!({
        "success": true,
        "characters": characters
    }))
}

/// Select character command - sends the selected character slot to Bevy
#[tauri::command]
pub async fn select_character(
    slot: u8,
    app_bridge: State<'_, AppBridge>,
) -> Result<serde_json::Value, String> {
    app_bridge.select_character(slot).await?;
    Ok(serde_json::json!({ "success": true }))
}

/// Create character command - sends character creation data to Bevy
#[tauri::command]
pub async fn create_character(
    request: CreateCharacterRequest,
    app_bridge: State<'_, AppBridge>,
) -> Result<serde_json::Value, String> {
    let _character = app_bridge
        .create_character(
            request.name,
            request.slot,
            request.hair_style,
            request.hair_color,
            request.sex,
        )
        .await?;
    Ok(serde_json::json!({ "success": true }))
}

/// Delete character command - sends character deletion request to Bevy
#[tauri::command]
pub async fn delete_character(
    char_id: u32,
    app_bridge: State<'_, AppBridge>,
) -> Result<serde_json::Value, String> {
    app_bridge.delete_character(char_id).await?;
    Ok(serde_json::json!({ "success": true }))
}
