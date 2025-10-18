use bevy::prelude::*;
use secrecy::SecretString;

use super::request_id::RequestId;

// ============================================================================
// Authentication Events
// ============================================================================

/// Event requesting login with server
#[derive(Event)]
pub struct LoginRequestedEvent {
    pub request_id: RequestId,
    pub username: String,
    pub password: SecretString,
}

/// Event requesting server selection
#[derive(Event)]
pub struct ServerSelectionRequestedEvent {
    pub request_id: RequestId,
    pub server_index: usize,
}

// ============================================================================
// Character Events
// ============================================================================

/// Event requesting character list
#[derive(Event)]
pub struct GetCharacterListRequestedEvent {
    pub request_id: RequestId,
}

/// Event requesting character selection
#[derive(Event)]
pub struct SelectCharacterRequestedEvent {
    pub request_id: RequestId,
    pub slot: u8,
}

/// Event requesting character creation
#[derive(Event)]
pub struct CreateCharacterRequestedEvent {
    pub request_id: RequestId,
    pub name: String,
    pub slot: u8,
    pub hair_style: u16,
    pub hair_color: u16,
    pub sex: u8,
}

/// Event requesting character deletion
#[derive(Event)]
pub struct DeleteCharacterRequestedEvent {
    pub request_id: RequestId,
    pub char_id: u32,
}

// ============================================================================
// Customization Events
// ============================================================================

/// Event requesting hairstyle list for a gender
#[derive(Event)]
pub struct GetHairstylesRequestedEvent {
    pub request_id: RequestId,
    pub gender: u8,
}

// ============================================================================
// Input Events
// ============================================================================

/// Event forwarding keyboard input from UI
#[derive(Event)]
pub struct KeyboardInputEvent {
    pub code: String,
    pub pressed: bool,
}

/// Event forwarding mouse position from UI
#[derive(Event)]
pub struct MousePositionEvent {
    pub x: f32,
    pub y: f32,
}
