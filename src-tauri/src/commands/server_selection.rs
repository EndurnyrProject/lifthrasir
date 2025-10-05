use crate::bridge::AppBridge;
use tauri::State;

/// Server selection command that sends the selected server index to Bevy
/// Returns success on completion
#[tauri::command]
pub async fn select_server(
    server_index: usize,
    app_bridge: State<'_, AppBridge>,
) -> Result<serde_json::Value, String> {
    app_bridge.select_server(server_index).await?;
    Ok(serde_json::json!({ "success": true }))
}
