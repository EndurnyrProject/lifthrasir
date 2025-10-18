// Domain modules
pub mod authentication;
pub mod character;
pub mod customization;
pub mod input;
pub mod world;

// Shared infrastructure
pub mod app_bridge;
pub mod correlation;
pub mod demux;
pub mod event_writers;
pub mod events;
#[macro_use]
pub mod macros;
pub mod pending_senders;
pub mod request_id;

// Re-export authentication functionality
pub use authentication::{
    handle_login_request, handle_server_selection_request, write_login_failure_response,
    write_login_success_response, write_server_selection_response,
};

// Re-export character functionality
pub use character::{
    handle_create_character_request, handle_delete_character_request,
    handle_get_character_list_request, handle_select_character_request,
    write_character_creation_response, write_character_deletion_response,
    write_character_list_response, write_character_selection_response,
};

// Re-export customization functionality
pub use customization::handle_get_hairstyles_request;

// Re-export input functionality
pub use input::{handle_keyboard_input, handle_mouse_position};

// Re-export world functionality
pub use world::{emit_world_events, WorldEmitter};

// Re-export shared infrastructure
pub use app_bridge::{AppBridge, SessionData, TauriEventReceiver};
pub use correlation::{
    cleanup_stale_correlations, CharacterCorrelation, LoginCorrelation, ServerCorrelation,
};
pub use demux::demux_tauri_events;
pub use events::*;
pub use pending_senders::PendingSenders;
