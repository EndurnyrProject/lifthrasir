use bevy::prelude::*;
use game_engine::domain::character::events::CharacterInfoWithJobName;
use game_engine::domain::entities::character::components::CharacterInfo;
use game_engine::infrastructure::networking::protocol::login::types::ServerInfo;
use secrecy::SecretString;
use serde::Serialize;
use tokio::sync::oneshot;

#[derive(Debug, Clone, Serialize)]
pub struct SessionData {
    pub username: String,
    pub login_id1: u32,
    pub account_id: u32,
    pub login_id2: u32,
    pub sex: u8,
    pub servers: Vec<ServerInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HairstyleInfo {
    pub id: u16,
    pub available_colors: Vec<u16>,
}

pub use super::character::status_emitter::CharacterStatusPayload;

pub enum TauriIncomingEvent {
    Login {
        username: String,
        password: SecretString,
        response_tx: oneshot::Sender<Result<SessionData, String>>,
    },
    SelectServer {
        server_index: usize,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    GetCharacterList {
        response_tx: oneshot::Sender<Result<Vec<CharacterInfoWithJobName>, String>>,
    },
    SelectCharacter {
        slot: u8,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    CreateCharacter {
        name: String,
        slot: u8,
        hair_style: u16,
        hair_color: u16,
        sex: u8,
        response_tx: oneshot::Sender<Result<CharacterInfo, String>>,
    },
    DeleteCharacter {
        char_id: u32,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    GetHairstyles {
        gender: u8,
        response_tx: oneshot::Sender<Result<Vec<HairstyleInfo>, String>>,
    },
    KeyboardInput {
        code: String,
        pressed: bool,
    },
    MousePosition {
        x: f32,
        y: f32,
    },
    MouseClick {
        x: f32,
        y: f32,
    },
    CameraRotation {
        delta_x: f32,
        delta_y: f32,
    },
    GetCharacterStatus {
        response_tx: oneshot::Sender<Result<CharacterStatusPayload, String>>,
    },
}

#[derive(Resource, Clone)]
pub struct AppBridge {
    pub tauri_tx: flume::Sender<TauriIncomingEvent>,
}

impl AppBridge {
    pub fn new() -> (Self, flume::Receiver<TauriIncomingEvent>) {
        let (tx, rx) = flume::bounded(256);
        let bridge = Self { tauri_tx: tx };
        (bridge, rx)
    }

    pub fn send_login(
        &self,
        username: String,
        password: SecretString,
    ) -> oneshot::Receiver<Result<SessionData, String>> {
        let (response_tx, response_rx) = oneshot::channel();

        let _ = self.tauri_tx.send(TauriIncomingEvent::Login {
            username,
            password,
            response_tx,
        });

        response_rx
    }

    pub fn send_select_server(&self, server_index: usize) -> oneshot::Receiver<Result<(), String>> {
        let (response_tx, response_rx) = oneshot::channel();

        let _ = self.tauri_tx.send(TauriIncomingEvent::SelectServer {
            server_index,
            response_tx,
        });

        response_rx
    }

    pub fn send_get_character_list(
        &self,
    ) -> oneshot::Receiver<Result<Vec<CharacterInfoWithJobName>, String>> {
        let (response_tx, response_rx) = oneshot::channel();

        let _ = self
            .tauri_tx
            .send(TauriIncomingEvent::GetCharacterList { response_tx });

        response_rx
    }

    pub fn send_select_character(&self, slot: u8) -> oneshot::Receiver<Result<(), String>> {
        let (response_tx, response_rx) = oneshot::channel();

        let _ = self
            .tauri_tx
            .send(TauriIncomingEvent::SelectCharacter { slot, response_tx });

        response_rx
    }

    pub fn send_create_character(
        &self,
        name: String,
        slot: u8,
        hair_style: u16,
        hair_color: u16,
        sex: u8,
    ) -> oneshot::Receiver<Result<CharacterInfo, String>> {
        let (response_tx, response_rx) = oneshot::channel();

        let _ = self.tauri_tx.send(TauriIncomingEvent::CreateCharacter {
            name,
            slot,
            hair_style,
            hair_color,
            sex,
            response_tx,
        });

        response_rx
    }

    pub fn send_delete_character(&self, char_id: u32) -> oneshot::Receiver<Result<(), String>> {
        let (response_tx, response_rx) = oneshot::channel();

        let _ = self.tauri_tx.send(TauriIncomingEvent::DeleteCharacter {
            char_id,
            response_tx,
        });

        response_rx
    }

    pub fn send_get_hairstyles(
        &self,
        gender: u8,
    ) -> oneshot::Receiver<Result<Vec<HairstyleInfo>, String>> {
        let (response_tx, response_rx) = oneshot::channel();

        let _ = self.tauri_tx.send(TauriIncomingEvent::GetHairstyles {
            gender,
            response_tx,
        });

        response_rx
    }

    pub fn forward_keyboard_input(&self, code: String, pressed: bool) -> Result<(), String> {
        self.tauri_tx
            .send(TauriIncomingEvent::KeyboardInput { code, pressed })
            .map_err(|e| format!("Failed to send keyboard input event: {}", e))?;

        Ok(())
    }

    pub fn forward_mouse_position(&self, x: f32, y: f32) -> Result<(), String> {
        self.tauri_tx
            .try_send(TauriIncomingEvent::MousePosition { x, y })
            .ok();

        Ok(())
    }

    pub fn forward_mouse_click(&self, x: f32, y: f32) -> Result<(), String> {
        self.tauri_tx
            .send(TauriIncomingEvent::MouseClick { x, y })
            .map_err(|e| format!("Failed to send mouse click event: {}", e))?;

        Ok(())
    }

    pub fn forward_camera_rotation(&self, delta_x: f32, delta_y: f32) -> Result<(), String> {
        self.tauri_tx
            .try_send(TauriIncomingEvent::CameraRotation { delta_x, delta_y })
            .ok();

        Ok(())
    }

    pub fn send_get_character_status(
        &self,
    ) -> oneshot::Receiver<Result<CharacterStatusPayload, String>> {
        let (response_tx, response_rx) = oneshot::channel();

        let _ = self
            .tauri_tx
            .send(TauriIncomingEvent::GetCharacterStatus { response_tx });

        response_rx
    }
}

#[derive(Resource)]
pub struct TauriEventReceiver(pub flume::Receiver<TauriIncomingEvent>);
