use crate::bridge::AppBridge;
use secrecy::SecretString;
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
    // Call send_login which returns a oneshot::Receiver
    let receiver = app_bridge.send_login(request.username, SecretString::from(request.password));

    // Await the response with timeout
    let session_data = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        receiver
    ).await {
        Ok(Ok(result)) => result?,
        Ok(Err(_)) => return Err("Response channel closed".into()),
        Err(_) => return Err("Login request timed out".into()),
    };

    Ok(serde_json::json!({
        "success": true,
        "session_data": session_data
    }))
}
