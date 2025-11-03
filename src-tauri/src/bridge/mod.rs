pub mod authentication;
pub mod character;
pub mod customization;
pub mod input;
pub mod world;

pub mod app_bridge;
pub mod correlation;
pub mod demux;
pub mod event_writers;
pub mod events;
#[macro_use]
pub mod macros;

pub use authentication::{
    handle_login_request, handle_server_selection_request, write_login_failure_response,
    write_login_success_response, write_server_selection_response,
};

pub use character::{
    handle_create_character_request, handle_delete_character_request,
    handle_get_character_list_request, handle_select_character_request,
    write_character_creation_response, write_character_deletion_response,
    write_character_list_response, write_character_selection_response,
};

pub use customization::handle_get_hairstyles_request;

pub use input::{
    emit_cursor_changes, emit_entity_names, handle_camera_rotation, handle_keyboard_input,
    handle_mouse_click, handle_mouse_position,
};

pub use world::{emit_world_events, WorldEmitter};

pub use app_bridge::{AppBridge, SessionData, TauriEventReceiver};
pub use correlation::{
    cleanup_stale_correlations, CharacterCorrelation, LoginCorrelation,
    PendingCharacterListSenders, PendingHairstyleSenders, ServerCorrelation,
};
pub use demux::demux_tauri_events;
pub use events::*;
