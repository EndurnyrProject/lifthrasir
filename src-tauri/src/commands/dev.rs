use tauri::{AppHandle, Manager};

/// Open the browser dev tools for debugging
#[tauri::command]
pub fn open_devtools(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        window.open_devtools();
    }
}

/// Close the browser dev tools
#[tauri::command]
pub fn close_devtools(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        window.close_devtools();
    }
}
