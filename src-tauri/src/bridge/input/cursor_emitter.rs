use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_system;
use game_engine::domain::input::CurrentCursorType;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::plugin::TauriSystems;

#[derive(Serialize, Clone)]
struct CursorChangePayload {
    cursor_type: String,
}

#[auto_add_system(
    plugin = crate::plugin::TauriIntegrationAutoPlugin,
    schedule = Update,
    config(in_set = TauriSystems::Emitters)
)]
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
