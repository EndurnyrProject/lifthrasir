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
    let receiver = app_bridge.send_get_character_list();

    let characters = match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        receiver
    ).await {
        Ok(Ok(result)) => result?,
        Ok(Err(_)) => return Err("Response channel closed".into()),
        Err(_) => return Err("Get character list request timed out".into()),
    };

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
    let receiver = app_bridge.send_select_character(slot);

    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        receiver
    ).await {
        Ok(Ok(result)) => result?,
        Ok(Err(_)) => return Err("Response channel closed".into()),
        Err(_) => return Err("Select character request timed out".into()),
    };

    Ok(serde_json::json!({ "success": true }))
}

/// Create character command - sends character creation data to Bevy
#[tauri::command]
pub async fn create_character(
    request: CreateCharacterRequest,
    app_bridge: State<'_, AppBridge>,
) -> Result<serde_json::Value, String> {
    let receiver = app_bridge.send_create_character(
        request.name,
        request.slot,
        request.hair_style,
        request.hair_color,
        request.sex,
    );

    let _character = match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        receiver
    ).await {
        Ok(Ok(result)) => result?,
        Ok(Err(_)) => return Err("Response channel closed".into()),
        Err(_) => return Err("Create character request timed out".into()),
    };

    Ok(serde_json::json!({ "success": true }))
}

/// Delete character command - sends character deletion request to Bevy
#[tauri::command]
pub async fn delete_character(
    char_id: u32,
    app_bridge: State<'_, AppBridge>,
) -> Result<serde_json::Value, String> {
    let receiver = app_bridge.send_delete_character(char_id);

    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        receiver
    ).await {
        Ok(Ok(result)) => result?,
        Ok(Err(_)) => return Err("Response channel closed".into()),
        Err(_) => return Err("Delete character request timed out".into()),
    };

    Ok(serde_json::json!({ "success": true }))
}
