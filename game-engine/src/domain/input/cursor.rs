use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::{auto_add_system, auto_init_resource};

use super::events::CursorChangeRequest;

/// Enum representing different cursor types based on game state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CursorType {
    Default,
    Add,
    Attack,
    Impossible,
    Talk,
}

impl CursorType {
    /// Convert cursor type to string representation for IPC
    pub fn as_str(&self) -> &'static str {
        match self {
            CursorType::Default => "default",
            CursorType::Add => "add",
            CursorType::Attack => "attack",
            CursorType::Impossible => "impossible",
            CursorType::Talk => "talk",
        }
    }
}

/// Resource tracking the current cursor type
#[derive(Resource, Debug)]
#[auto_init_resource(plugin = crate::app::input_plugin::InputPlugin)]
pub struct CurrentCursorType {
    cursor_type: CursorType,
}

impl CurrentCursorType {
    /// Create a new CurrentCursorType with default cursor
    pub fn new() -> Self {
        Self {
            cursor_type: CursorType::Default,
        }
    }

    /// Get the current cursor type
    pub fn get(&self) -> CursorType {
        self.cursor_type
    }

    /// Set the cursor type, returns true if changed
    pub fn set(&mut self, new_type: CursorType) -> bool {
        if self.cursor_type != new_type {
            self.cursor_type = new_type;
            true
        } else {
            false
        }
    }
}

impl Default for CurrentCursorType {
    fn default() -> Self {
        Self::new()
    }
}

/// System to handle cursor change requests
#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = Update,
    config(after = crate::domain::input::systems::update_cursor_for_terrain)
)]
pub fn handle_cursor_change_requests(
    mut current_cursor: ResMut<CurrentCursorType>,
    mut messages: MessageReader<CursorChangeRequest>,
) {
    if let Some(last_message) = messages.read().last() {
        if current_cursor.set(last_message.cursor_type) {
            trace!("Cursor changed to: {:?}", last_message.cursor_type);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::MessageWriter;

    #[test]
    fn test_cursor_type_as_str() {
        assert_eq!(CursorType::Default.as_str(), "default");
        assert_eq!(CursorType::Add.as_str(), "add");
        assert_eq!(CursorType::Attack.as_str(), "attack");
        assert_eq!(CursorType::Impossible.as_str(), "impossible");
        assert_eq!(CursorType::Talk.as_str(), "talk");
    }

    #[test]
    fn test_current_cursor_type_default() {
        let cursor = CurrentCursorType::default();
        assert_eq!(cursor.get(), CursorType::Default);
    }

    #[test]
    fn test_current_cursor_type_set() {
        let mut cursor = CurrentCursorType::new();
        assert_eq!(cursor.get(), CursorType::Default);

        assert!(cursor.set(CursorType::Attack));
        assert_eq!(cursor.get(), CursorType::Attack);

        assert!(!cursor.set(CursorType::Attack));
        assert_eq!(cursor.get(), CursorType::Attack);

        assert!(cursor.set(CursorType::Default));
        assert_eq!(cursor.get(), CursorType::Default);
    }

    #[test]
    fn test_cursor_change_request() {
        let request = CursorChangeRequest::new(CursorType::Talk);
        assert_eq!(request.cursor_type, CursorType::Talk);
    }

    #[test]
    fn test_handle_cursor_change_requests_multiple_messages() {
        let mut app = App::new();
        app.init_resource::<CurrentCursorType>();
        app.add_message::<CursorChangeRequest>();

        app.add_systems(
            Update,
            (
                |mut writer: MessageWriter<CursorChangeRequest>| {
                    writer.write(CursorChangeRequest::new(CursorType::Add));
                    writer.write(CursorChangeRequest::new(CursorType::Attack));
                    writer.write(CursorChangeRequest::new(CursorType::Talk));
                },
                handle_cursor_change_requests,
            )
                .chain(),
        );

        app.update();

        let current = app.world().resource::<CurrentCursorType>();
        assert_eq!(current.get(), CursorType::Talk);
    }

    #[test]
    fn test_handle_cursor_change_requests_no_messages() {
        let mut app = App::new();
        app.init_resource::<CurrentCursorType>();
        app.add_message::<CursorChangeRequest>();
        app.add_systems(Update, handle_cursor_change_requests);

        app.update();

        let current = app.world().resource::<CurrentCursorType>();
        assert_eq!(current.get(), CursorType::Default);
    }
}
