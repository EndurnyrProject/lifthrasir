use tauri::{AppHandle, Manager};

/// Open the browser dev tools for debugging
#[tauri::command]
pub fn open_devtools(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        #[cfg(debug_assertions)]
        window.open_devtools();
    }
}

/// Close the browser dev tools
#[tauri::command]
pub fn close_devtools(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        #[cfg(debug_assertions)]
        window.close_devtools();
    }
}

/// WORKAROUND: macOS 26 WebKit layer compositing bug
/// Force window refresh by toggling size, decorations, and shadow
#[tauri::command]
pub fn refresh_window(window: tauri::Window) -> Result<(), String> {
    let current_size = window.outer_size().map_err(|e| e.to_string())?;

    let temp_size = tauri::Size::Physical(tauri::PhysicalSize::new(
        current_size.width + 1,
        current_size.height + 1,
    ));

    window.set_size(temp_size).map_err(|e| e.to_string())?;
    window.set_size(current_size).map_err(|e| e.to_string())?;

    window.set_shadow(false).map_err(|e| e.to_string())?;
    window
        .set_background_color(Some(tauri::window::Color(0, 0, 0, 0)))
        .map_err(|e| e.to_string())?;
    window.set_decorations(false).map_err(|e| e.to_string())?;

    window.set_focus().map_err(|e| e.to_string())?;

    let handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let _ = window.set_decorations(true).map_err(|e| e.to_string());
        let _ = window.set_shadow(true);
    });
    handle.join().unwrap();

    Ok(())
}
