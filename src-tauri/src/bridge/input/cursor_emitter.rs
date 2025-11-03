use bevy::prelude::*;
use game_engine::domain::input::CurrentCursorType;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
struct CursorChangePayload {
    cursor_type: String,
}

/// System that emits cursor changes to the Tauri frontend
///
/// Polls CurrentCursorType resource for changes and emits cursor-change
/// events that the React frontend can consume to update the CSS cursor
pub fn emit_cursor_changes(app_handle: NonSend<AppHandle>, current_cursor: Res<CurrentCursorType>) {
    if !current_cursor.is_changed() {
        return;
    }

    let payload = CursorChangePayload {
        cursor_type: current_cursor.get().as_str().to_string(),
    };

    if let Err(e) = app_handle.emit("cursor-change", payload) {
        error!("Failed to emit cursor-change event: {:?}", e);
    }
}
