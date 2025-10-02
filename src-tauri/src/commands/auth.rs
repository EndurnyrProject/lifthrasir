use crate::bridge::AppBridge;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Login command that sends credentials to Bevy for authentication
/// Returns session data on success wrapped in a response object
#[tauri::command]
pub async fn login(
    request: LoginRequest,
    app_bridge: State<'_, AppBridge>,
) -> Result<serde_json::Value, String> {
    let session_data = app_bridge.login(request.username, request.password).await?;

    // Wrap in the format the frontend expects
    Ok(serde_json::json!({
        "success": true,
        "session_data": session_data
    }))
}
