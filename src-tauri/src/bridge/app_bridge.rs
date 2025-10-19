use bevy::prelude::*;
use game_engine::domain::entities::character::components::CharacterInfo;
use game_engine::infrastructure::networking::protocol::login::types::ServerInfo;
use secrecy::SecretString;
use serde::Serialize;
use tokio::sync::oneshot;

use super::request_id::RequestId;

// ============================================================================
// Response Types
// ============================================================================

/// Session data returned on successful login
#[derive(Debug, Clone, Serialize)]
pub struct SessionData {
    pub username: String,
    pub login_id1: u32,
    pub account_id: u32,
    pub login_id2: u32,
    pub sex: u8,
    pub servers: Vec<ServerInfo>,
}

/// Hairstyle information
#[derive(Debug, Clone, Serialize)]
pub struct HairstyleInfo {
    pub id: u16,
    pub available_colors: Vec<u16>,
}

// ============================================================================
// Event Types (Internal to Bridge)
// ============================================================================

/// Internal events sent from Tauri commands to Bevy ECS via flume channel
/// These are demuxed into typed Bevy events by the demux system
/// Each event that needs a response includes the oneshot sender to send the response back
pub enum TauriIncomingEvent {
    /// Login event
    Login {
        request_id: RequestId,
        username: String,
        password: SecretString,
        response_tx: oneshot::Sender<Result<SessionData, String>>,
    },
    /// Server selection event
    SelectServer {
        request_id: RequestId,
        server_index: usize,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    /// Get character list event
    GetCharacterList {
        request_id: RequestId,
        response_tx: oneshot::Sender<Result<Vec<CharacterInfo>, String>>,
    },
    /// Select character event
    SelectCharacter {
        request_id: RequestId,
        slot: u8,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    /// Create character event
    CreateCharacter {
        request_id: RequestId,
        name: String,
        slot: u8,
        hair_style: u16,
        hair_color: u16,
        sex: u8,
        response_tx: oneshot::Sender<Result<CharacterInfo, String>>,
    },
    /// Delete character event
    DeleteCharacter {
        request_id: RequestId,
        char_id: u32,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    /// Get available hairstyles for a gender
    GetHairstyles {
        request_id: RequestId,
        gender: u8,
        response_tx: oneshot::Sender<Result<Vec<HairstyleInfo>, String>>,
    },
    /// Forward keyboard input from JavaScript to Bevy (no response)
    KeyboardInput { code: String, pressed: bool },
    /// Forward mouse position from JavaScript to Bevy (no response)
    MousePosition { x: f32, y: f32 },
}

// ============================================================================
// Bridge
// ============================================================================

/// Bridge between Tauri commands and Bevy ECS
/// This resource allows async Tauri commands to communicate with Bevy systems
#[derive(Resource, Clone)]
pub struct AppBridge {
    /// Send events from Tauri → Bevy
    pub tauri_tx: flume::Sender<TauriIncomingEvent>,
}

impl AppBridge {
    /// Create a new AppBridge and return the receiver for Bevy to consume
    pub fn new() -> (Self, flume::Receiver<TauriIncomingEvent>) {
        // Bounded channel for backpressure
        // 256 events should be more than enough for UI interactions
        let (tx, rx) = flume::bounded(256);
        let bridge = Self { tauri_tx: tx };
        (bridge, rx)
    }

    /// Send a login event and return receiver for response
    pub fn send_login(
        &self,
        username: String,
        password: SecretString,
    ) -> oneshot::Receiver<Result<SessionData, String>> {
        let request_id = RequestId::new();
        let (response_tx, response_rx) = oneshot::channel();

        // Send event with the response sender - if channel is full, this will block until space is available
        let _ = self.tauri_tx.send(TauriIncomingEvent::Login {
            request_id,
            username,
            password,
            response_tx,
        });

        response_rx
    }

    /// Send a server selection event and return receiver for response
    pub fn send_select_server(&self, server_index: usize) -> oneshot::Receiver<Result<(), String>> {
        let request_id = RequestId::new();
        let (response_tx, response_rx) = oneshot::channel();

        let _ = self.tauri_tx.send(TauriIncomingEvent::SelectServer {
            request_id,
            server_index,
            response_tx,
        });

        response_rx
    }

    /// Send get character list event and return receiver for response
    pub fn send_get_character_list(&self) -> oneshot::Receiver<Result<Vec<CharacterInfo>, String>> {
        let request_id = RequestId::new();
        let (response_tx, response_rx) = oneshot::channel();

        let _ = self.tauri_tx.send(TauriIncomingEvent::GetCharacterList {
            request_id,
            response_tx,
        });

        response_rx
    }

    /// Send select character event and return receiver for response
    pub fn send_select_character(&self, slot: u8) -> oneshot::Receiver<Result<(), String>> {
        let request_id = RequestId::new();
        let (response_tx, response_rx) = oneshot::channel();

        let _ = self.tauri_tx.send(TauriIncomingEvent::SelectCharacter {
            request_id,
            slot,
            response_tx,
        });

        response_rx
    }

    /// Send create character event and return receiver for response
    pub fn send_create_character(
        &self,
        name: String,
        slot: u8,
        hair_style: u16,
        hair_color: u16,
        sex: u8,
    ) -> oneshot::Receiver<Result<CharacterInfo, String>> {
        let request_id = RequestId::new();
        let (response_tx, response_rx) = oneshot::channel();

        let _ = self.tauri_tx.send(TauriIncomingEvent::CreateCharacter {
            request_id,
            name,
            slot,
            hair_style,
            hair_color,
            sex,
            response_tx,
        });

        response_rx
    }

    /// Send delete character event and return receiver for response
    pub fn send_delete_character(&self, char_id: u32) -> oneshot::Receiver<Result<(), String>> {
        let request_id = RequestId::new();
        let (response_tx, response_rx) = oneshot::channel();

        let _ = self.tauri_tx.send(TauriIncomingEvent::DeleteCharacter {
            request_id,
            char_id,
            response_tx,
        });

        response_rx
    }

    /// Send get hairstyles event and return receiver for response
    pub fn send_get_hairstyles(
        &self,
        gender: u8,
    ) -> oneshot::Receiver<Result<Vec<HairstyleInfo>, String>> {
        let request_id = RequestId::new();
        let (response_tx, response_rx) = oneshot::channel();

        let _ = self.tauri_tx.send(TauriIncomingEvent::GetHairstyles {
            request_id,
            gender,
            response_tx,
        });

        response_rx
    }

    /// Forward keyboard input from JavaScript to Bevy (fire and forget)
    pub fn forward_keyboard_input(&self, code: String, pressed: bool) -> Result<(), String> {
        self.tauri_tx
            .send(TauriIncomingEvent::KeyboardInput { code, pressed })
            .map_err(|e| format!("Failed to send keyboard input event: {}", e))?;

        Ok(())
    }

    /// Forward mouse position from JavaScript to Bevy (fire and forget)
    pub fn forward_mouse_position(&self, x: f32, y: f32) -> Result<(), String> {
        self.tauri_tx
            .send(TauriIncomingEvent::MousePosition { x, y })
            .map_err(|e| format!("Failed to send mouse position event: {}", e))?;

        Ok(())
    }
}

/// Wrapper resource for the Tauri event receiver (flume channel)
#[derive(Resource)]
pub struct TauriEventReceiver(pub flume::Receiver<TauriIncomingEvent>);
