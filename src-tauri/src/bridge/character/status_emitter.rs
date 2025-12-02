use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_system;
use game_engine::domain::entities::character::components::core::CharacterData;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::character::events::status_events::StatusParameterChanged;
use game_engine::domain::entities::components::EntityName;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::infrastructure::lua_scripts::job::registry::JobSpriteRegistry;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::bridge::correlation::PendingCharacterStatusSenders;
use crate::bridge::events::GetCharacterStatusRequestedEvent;
use crate::plugin::TauriSystems;

#[derive(Serialize, Clone, Debug)]
pub struct CharacterStatusPayload {
    pub name: String,
    pub job_name: String,
    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
    pub base_level: u32,
    pub job_level: u32,
    pub base_exp: u32,
    pub next_base_exp: u32,
    pub job_exp: u32,
    pub next_job_exp: u32,
    pub zeny: u32,
    pub weight: u32,
    pub max_weight: u32,
}

fn build_character_status_payload(
    status: &CharacterStatus,
    char_data: &CharacterData,
    maybe_name: Option<&EntityName>,
    job_registry: Option<&JobSpriteRegistry>,
) -> CharacterStatusPayload {
    let name = maybe_name
        .map(|n| n.name.clone())
        .unwrap_or_else(|| "Loading...".to_string());

    let job_name = job_registry
        .and_then(|registry| registry.get_display_name(char_data.job_id as u32))
        .unwrap_or("Unknown")
        .to_string();

    CharacterStatusPayload {
        name,
        job_name,
        hp: status.hp,
        max_hp: status.max_hp,
        sp: status.sp,
        max_sp: status.max_sp,
        base_level: status.base_level,
        job_level: status.job_level,
        base_exp: status.base_exp,
        next_base_exp: status.next_base_exp,
        job_exp: status.job_exp,
        next_job_exp: status.next_job_exp,
        zeny: status.zeny,
        weight: status.weight,
        max_weight: status.max_weight,
    }
}

#[auto_add_system(
    plugin = crate::plugin::TauriIntegrationAutoPlugin,
    schedule = Update,
    config(in_set = TauriSystems::ResponseWriters)
)]
pub fn write_character_status_response(
    mut request_events: MessageReader<GetCharacterStatusRequestedEvent>,
    mut status_senders: ResMut<PendingCharacterStatusSenders>,
    job_registry: Option<Res<JobSpriteRegistry>>,
    query: Query<(&CharacterStatus, &CharacterData, Option<&EntityName>), With<LocalPlayer>>,
) {
    for _ in request_events.read() {
        if let Some(sender) = status_senders.pop_oldest() {
            match query.single() {
                Ok((status, char_data, maybe_name)) => {
                    let payload = build_character_status_payload(
                        status,
                        char_data,
                        maybe_name,
                        job_registry.as_deref(),
                    );

                    debug!("Sending character status response to UI");
                    if sender.send(Ok(payload)).is_err() {
                        debug!("Failed to send character status response - receiver was dropped");
                    }
                }
                Err(e) => {
                    debug!("Failed to get character status: {:?}", e);
                    let _ = sender.send(Err("Character not loaded".to_string()));
                }
            }
        }
    }
}

#[auto_add_system(
    plugin = crate::plugin::TauriIntegrationAutoPlugin,
    schedule = Update,
    config(in_set = TauriSystems::ResponseWriters)
)]
pub fn emit_character_status_system(
    app_handle: NonSend<AppHandle>,
    mut status_events: MessageReader<StatusParameterChanged>,
    job_registry: Option<Res<JobSpriteRegistry>>,
    query: Query<(&CharacterStatus, &CharacterData, Option<&EntityName>), With<LocalPlayer>>,
) {
    if status_events.is_empty() {
        return;
    }

    status_events.read().for_each(drop);

    let (status, char_data, maybe_name) = match query.single() {
        Ok(result) => result,
        Err(e) => {
            debug!("Failed to emit character status: {:?}", e);
            return;
        }
    };

    let payload = build_character_status_payload(
        status,
        char_data,
        maybe_name,
        job_registry.as_deref(),
    );

    if let Err(e) = app_handle.emit("character-status-update", payload) {
        error!("Failed to emit character-status-update event: {:?}", e);
    }
}
