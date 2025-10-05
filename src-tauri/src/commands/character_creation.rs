use crate::bridge::AppBridge;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
pub struct PreviewUpdateRequest {
    pub gender: u8,
    pub hair_style: u16,
    pub hair_color: u16,
}

/// Get available hairstyles for a specific gender
#[tauri::command]
pub async fn get_hairstyles(
    gender: u8,
    app_bridge: State<'_, AppBridge>,
) -> Result<serde_json::Value, String> {
    let hairstyles = app_bridge.get_hairstyles(gender).await?;
    Ok(serde_json::json!({
        "success": true,
        "hairstyles": hairstyles
    }))
}

/// Update the character creation preview
#[tauri::command]
pub async fn update_creation_preview(
    preview: PreviewUpdateRequest,
    app_bridge: State<'_, AppBridge>,
) -> Result<(), String> {
    app_bridge
        .update_creation_preview(preview.gender, preview.hair_style, preview.hair_color)
        .await
}

/// Enter character creation screen
#[tauri::command]
pub async fn enter_character_creation(
    slot: u8,
    app_bridge: State<'_, AppBridge>,
) -> Result<(), String> {
    app_bridge.enter_character_creation(slot).await
}

/// Exit character creation screen
#[tauri::command]
pub async fn exit_character_creation(app_bridge: State<'_, AppBridge>) -> Result<(), String> {
    app_bridge.exit_character_creation().await
}
