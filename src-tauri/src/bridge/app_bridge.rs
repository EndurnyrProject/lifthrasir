use bevy::prelude::*;
use game_engine::domain::character::CharacterData;
use game_engine::infrastructure::networking::protocols::ro_login::ServerInfo;
use serde::Serialize;
use tokio::sync::{mpsc, oneshot};

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
// Event Types
// ============================================================================

/// Events sent from Tauri commands to Bevy ECS
pub enum TauriEvent {
    /// Login event
    Login {
        username: String,
        password: String,
        response_tx: oneshot::Sender<Result<SessionData, String>>,
    },
    /// Server selection event
    SelectServer {
        server_index: usize,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    /// Get character list event
    GetCharacterList {
        response_tx: oneshot::Sender<Result<Vec<CharacterData>, String>>,
    },
    /// Select character event
    SelectCharacter {
        slot: u8,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    /// Create character event
    CreateCharacter {
        name: String,
        slot: u8,
        hair_style: u16,
        hair_color: u16,
        sex: u8,
        response_tx: oneshot::Sender<Result<CharacterData, String>>,
    },
    /// Delete character event
    DeleteCharacter {
        char_id: u32,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    /// Get available hairstyles for a gender
    GetHairstyles {
        gender: u8,
        response_tx: oneshot::Sender<Result<Vec<HairstyleInfo>, String>>,
    },
    /// Forward keyboard input from JavaScript to Bevy
    KeyboardInput {
        code: String,
        pressed: bool,
    },
    /// Forward mouse position from JavaScript to Bevy
    MousePosition {
        x: f32,
        y: f32,
    },
}

// ============================================================================
// Bridge
// ============================================================================

/// Bridge between Tauri commands and Bevy ECS
/// This resource allows async Tauri commands to communicate with Bevy systems
#[derive(Resource, Clone)]
pub struct AppBridge {
    /// Send events from Tauri → Bevy
    pub tauri_tx: mpsc::UnboundedSender<TauriEvent>,
}

impl AppBridge {
    /// Create a new AppBridge and return the receiver for Bevy to consume
    pub fn new() -> (Self, mpsc::UnboundedReceiver<TauriEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let bridge = Self { tauri_tx: tx };
        (bridge, rx)
    }

    /// Send a login event and wait for response
    pub async fn login(&self, username: String, password: String) -> Result<SessionData, String> {
        let (tx, rx) = oneshot::channel();

        self.tauri_tx
            .send(TauriEvent::Login {
                username,
                password,
                response_tx: tx,
            })
            .map_err(|e| format!("Failed to send login event: {}", e))?;

        // Wait for response with 30 second timeout
        tokio::time::timeout(tokio::time::Duration::from_secs(30), rx)
            .await
            .map_err(|_| "Login timeout - server not responding".to_string())?
            .map_err(|_| "Response channel closed unexpectedly".to_string())?
    }

    /// Send a server selection event and wait for response
    pub async fn select_server(&self, server_index: usize) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();

        self.tauri_tx
            .send(TauriEvent::SelectServer {
                server_index,
                response_tx: tx,
            })
            .map_err(|e| format!("Failed to send server selection event: {}", e))?;

        // Wait for response with 10 second timeout
        tokio::time::timeout(tokio::time::Duration::from_secs(10), rx)
            .await
            .map_err(|_| "Server selection timeout".to_string())?
            .map_err(|_| "Response channel closed unexpectedly".to_string())?
    }

    /// Get character list event and wait for response
    pub async fn get_character_list(&self) -> Result<Vec<CharacterData>, String> {
        let (tx, rx) = oneshot::channel();

        self.tauri_tx
            .send(TauriEvent::GetCharacterList { response_tx: tx })
            .map_err(|e| format!("Failed to send get character list event: {}", e))?;

        // Wait for response with 10 second timeout
        tokio::time::timeout(tokio::time::Duration::from_secs(10), rx)
            .await
            .map_err(|_| "Get character list timeout".to_string())?
            .map_err(|_| "Response channel closed unexpectedly".to_string())?
    }

    /// Select character event and wait for response
    pub async fn select_character(&self, slot: u8) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();

        self.tauri_tx
            .send(TauriEvent::SelectCharacter {
                slot,
                response_tx: tx,
            })
            .map_err(|e| format!("Failed to send select character event: {}", e))?;

        // Wait for response with 10 second timeout
        tokio::time::timeout(tokio::time::Duration::from_secs(10), rx)
            .await
            .map_err(|_| "Select character timeout".to_string())?
            .map_err(|_| "Response channel closed unexpectedly".to_string())?
    }

    /// Create character event and wait for response
    pub async fn create_character(
        &self,
        name: String,
        slot: u8,
        hair_style: u16,
        hair_color: u16,
        sex: u8,
    ) -> Result<CharacterData, String> {
        let (tx, rx) = oneshot::channel();

        self.tauri_tx
            .send(TauriEvent::CreateCharacter {
                name,
                slot,
                hair_style,
                hair_color,
                sex,
                response_tx: tx,
            })
            .map_err(|e| format!("Failed to send create character event: {}", e))?;

        // Wait for response with 15 second timeout (creation can take longer)
        tokio::time::timeout(tokio::time::Duration::from_secs(15), rx)
            .await
            .map_err(|_| "Create character timeout".to_string())?
            .map_err(|_| "Response channel closed unexpectedly".to_string())?
    }

    /// Delete character event and wait for response
    pub async fn delete_character(&self, char_id: u32) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();

        self.tauri_tx
            .send(TauriEvent::DeleteCharacter {
                char_id,
                response_tx: tx,
            })
            .map_err(|e| format!("Failed to send delete character event: {}", e))?;

        // Wait for response with 10 second timeout
        tokio::time::timeout(tokio::time::Duration::from_secs(10), rx)
            .await
            .map_err(|_| "Delete character timeout".to_string())?
            .map_err(|_| "Response channel closed unexpectedly".to_string())?
    }

    /// Get available hairstyles for a gender
    pub async fn get_hairstyles(&self, gender: u8) -> Result<Vec<HairstyleInfo>, String> {
        let (tx, rx) = oneshot::channel();

        self.tauri_tx
            .send(TauriEvent::GetHairstyles {
                gender,
                response_tx: tx,
            })
            .map_err(|e| format!("Failed to send get hairstyles event: {}", e))?;

        // Wait for response with 5 second timeout (quick operation)
        tokio::time::timeout(tokio::time::Duration::from_secs(5), rx)
            .await
            .map_err(|_| "Get hairstyles timeout".to_string())?
            .map_err(|_| "Response channel closed unexpectedly".to_string())?
    }

    /// Forward keyboard input from JavaScript to Bevy (fire and forget)
    pub fn forward_keyboard_input(&self, code: String, pressed: bool) -> Result<(), String> {
        self.tauri_tx
            .send(TauriEvent::KeyboardInput { code, pressed })
            .map_err(|e| format!("Failed to send keyboard input event: {}", e))?;

        Ok(())
    }

    /// Forward mouse position from JavaScript to Bevy (fire and forget)
    pub fn forward_mouse_position(&self, x: f32, y: f32) -> Result<(), String> {
        self.tauri_tx
            .send(TauriEvent::MousePosition { x, y })
            .map_err(|e| format!("Failed to send mouse position event: {}", e))?;

        Ok(())
    }
}

/// Wrapper resource for the Tauri event receiver
#[derive(Resource)]
pub struct TauriEventReceiver(pub mpsc::UnboundedReceiver<TauriEvent>);
