use crate::bridge::AppBridge;
use tauri::State;

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
