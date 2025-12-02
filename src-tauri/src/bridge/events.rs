use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_message;
use secrecy::SecretString;

#[derive(Message)]
#[auto_add_message(plugin = crate::plugin::TauriIntegrationAutoPlugin)]
pub struct LoginRequestedEvent {
    pub username: String,
    pub password: SecretString,
}

#[derive(Message)]
#[auto_add_message(plugin = crate::plugin::TauriIntegrationAutoPlugin)]
pub struct ServerSelectionRequestedEvent {
    pub server_index: usize,
}

#[derive(Message)]
#[auto_add_message(plugin = crate::plugin::TauriIntegrationAutoPlugin)]
pub struct GetCharacterListRequestedEvent {}

#[derive(Message)]
#[auto_add_message(plugin = crate::plugin::TauriIntegrationAutoPlugin)]
pub struct SelectCharacterRequestedEvent {
    pub slot: u8,
}

#[derive(Message)]
#[auto_add_message(plugin = crate::plugin::TauriIntegrationAutoPlugin)]
pub struct CreateCharacterRequestedEvent {
    pub name: String,
    pub slot: u8,
    pub hair_style: u16,
    pub hair_color: u16,
    pub sex: u8,
}

#[derive(Message)]
#[auto_add_message(plugin = crate::plugin::TauriIntegrationAutoPlugin)]
pub struct DeleteCharacterRequestedEvent {
    pub char_id: u32,
}

#[derive(Message)]
#[auto_add_message(plugin = crate::plugin::TauriIntegrationAutoPlugin)]
pub struct GetHairstylesRequestedEvent {
    pub gender: u8,
}

#[derive(Message)]
#[auto_add_message(plugin = crate::plugin::TauriIntegrationAutoPlugin)]
pub struct KeyboardInputEvent {
    pub code: String,
    pub pressed: bool,
}

#[derive(Message)]
#[auto_add_message(plugin = crate::plugin::TauriIntegrationAutoPlugin)]
pub struct MousePositionEvent {
    pub x: f32,
    pub y: f32,
}

#[derive(Message)]
#[auto_add_message(plugin = crate::plugin::TauriIntegrationAutoPlugin)]
pub struct MouseClickEvent {
    pub x: f32,
    pub y: f32,
}

#[derive(Message)]
#[auto_add_message(plugin = crate::plugin::TauriIntegrationAutoPlugin)]
pub struct CameraRotationEvent {
    pub delta_x: f32,
    pub delta_y: f32,
}

#[derive(Message)]
#[auto_add_message(plugin = crate::plugin::TauriIntegrationAutoPlugin)]
pub struct GetCharacterStatusRequestedEvent;

#[derive(Message)]
#[auto_add_message(plugin = crate::plugin::TauriIntegrationAutoPlugin)]
pub struct ChatRequestedEvent {
    pub message: String,
}
