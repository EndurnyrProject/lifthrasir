use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

use super::cursor::CursorType;

/// Message requesting a cursor change
#[derive(Message, Debug, Clone, Copy)]
#[auto_add_message(plugin = crate::app::input_plugin::InputPlugin)]
pub struct CursorChangeRequest {
    pub cursor_type: CursorType,
}

impl CursorChangeRequest {
    pub fn new(cursor_type: CursorType) -> Self {
        Self { cursor_type }
    }
}
