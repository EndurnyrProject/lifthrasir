use crate::bridge::AppBridge;
use tauri::State;

/// Server selection command that sends the selected server index to Bevy
/// Returns success on completion
#[tauri::command]
pub async fn select_server(
    server_index: usize,
    app_bridge: State<'_, AppBridge>,
) -> Result<serde_json::Value, String> {
    // Call send_select_server which returns a oneshot::Receiver
    let receiver = app_bridge.send_select_server(server_index);

    // Await the response with timeout
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        receiver
    ).await {
        Ok(Ok(result)) => result?,
        Ok(Err(_)) => return Err("Response channel closed".into()),
        Err(_) => return Err("Server selection request timed out".into()),
    };

    Ok(serde_json::json!({ "success": true }))
}
