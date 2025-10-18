use crate::bridge::AppBridge;
use tauri::State;

/// Get available hairstyles for a specific gender
#[tauri::command]
pub async fn get_hairstyles(
    gender: u8,
    app_bridge: State<'_, AppBridge>,
) -> Result<serde_json::Value, String> {
    let receiver = app_bridge.send_get_hairstyles(gender);

    let hairstyles = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        receiver
    ).await {
        Ok(Ok(result)) => result?,
        Ok(Err(_)) => return Err("Response channel closed".into()),
        Err(_) => return Err("Get hairstyles request timed out".into()),
    };

    Ok(serde_json::json!({
        "success": true,
        "hairstyles": hairstyles
    }))
}
