use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_event;

use super::cursor::CursorType;

/// Message requesting a cursor change
#[derive(Message, Debug, Clone, Copy)]
#[auto_add_event(plugin = crate::app::input_plugin::InputPlugin)]
pub struct CursorChangeRequest {
    pub cursor_type: CursorType,
}

impl CursorChangeRequest {
    pub fn new(cursor_type: CursorType) -> Self {
        Self { cursor_type }
    }
}
