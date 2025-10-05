use super::{HairstyleInfo, TauriEvent, TauriEventReceiver};
use crate::bridge::pending_senders::PendingSenders;
use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::character::{
    catalog::HeadStyleCatalog, CharacterCreationForm, CloseCharacterCreationEvent,
    CreateCharacterRequestEvent, DeleteCharacterRequestEvent, Gender, JobClass,
    OpenCharacterCreationEvent, RequestCharacterListEvent, SelectCharacterEvent,
};
use game_engine::presentation::ui::character_creation::UpdateCharacterPreviewEvent;
use game_engine::infrastructure::networking::session::UserSession;
use game_engine::presentation::ui::events::{LoginAttemptEvent, ServerSelectedEvent};
use secrecy::SecretString;

/// System that translates Tauri events to Bevy domain events
/// Runs in the Update schedule, checking for new events from Tauri
pub fn translate_tauri_events(
    mut tauri_rx: ResMut<TauriEventReceiver>,
    mut login_events: EventWriter<LoginAttemptEvent>,
    mut server_events: EventWriter<ServerSelectedEvent>,
    mut char_list_events: EventWriter<RequestCharacterListEvent>,
    mut select_char_events: EventWriter<SelectCharacterEvent>,
    mut create_char_events: EventWriter<CreateCharacterRequestEvent>,
    mut delete_char_events: EventWriter<DeleteCharacterRequestEvent>,
    mut update_preview_events: EventWriter<UpdateCharacterPreviewEvent>,
    mut open_creation_events: EventWriter<OpenCharacterCreationEvent>,
    mut close_creation_events: EventWriter<CloseCharacterCreationEvent>,
    mut next_state: ResMut<NextState<GameState>>,
    mut pending: ResMut<PendingSenders>,
    session: Option<Res<UserSession>>,
    catalog: Option<Res<HeadStyleCatalog>>,
) {
    // Try to receive all pending events without blocking
    while let Ok(event) = tauri_rx.0.try_recv() {
        match event {
            TauriEvent::Login {
                username,
                password,
                response_tx,
            } => {
                pending.logins.senders.insert(username.clone(), response_tx);

                login_events.write(LoginAttemptEvent {
                    username,
                    password: SecretString::new(password.into_boxed_str()),
                });
            }
            TauriEvent::SelectServer {
                server_index,
                response_tx,
            } => {
                pending.servers.senders.insert(server_index, response_tx);

                if let Some(session) = session.as_ref() {
                    if let Some(server) = session.server_list.get(server_index) {
                        server_events.write(ServerSelectedEvent {
                            server: server.clone(),
                        });
                    }
                }
            }
            TauriEvent::GetCharacterList { response_tx } => {
                pending.char_lists.senders.push(response_tx);

                char_list_events.write(RequestCharacterListEvent);
            }
            TauriEvent::SelectCharacter { slot, response_tx } => {
                pending.char_selections.senders.insert(slot, response_tx);

                select_char_events.write(SelectCharacterEvent { slot });
            }
            TauriEvent::CreateCharacter {
                name,
                slot,
                hair_style,
                hair_color,
                sex,
                response_tx,
            } => {
                pending.char_creations.senders.insert(slot, response_tx);

                let form = CharacterCreationForm {
                    name,
                    slot,
                    hair_style,
                    hair_color,
                    starting_job: JobClass::Novice,
                    sex: Gender::from(sex),
                    str: 5,
                    agi: 5,
                    vit: 5,
                    int: 5,
                    dex: 5,
                    luk: 5,
                };

                create_char_events.write(CreateCharacterRequestEvent { form });
            }
            TauriEvent::DeleteCharacter {
                char_id,
                response_tx,
            } => {
                pending.char_deletions.senders.insert(char_id, response_tx);

                delete_char_events.write(DeleteCharacterRequestEvent {
                    character_id: char_id,
                });
            }
            TauriEvent::UpdateSpritePositions { .. } => {}
            TauriEvent::GetHairstyles {
                gender,
                response_tx,
            } => {
                let result = if let Some(catalog) = &catalog {
                    let gender_enum = Gender::from(gender);
                    let styles = catalog.get_all(gender_enum);

                    let hairstyles: Vec<HairstyleInfo> = styles
                        .iter()
                        .map(|entry| HairstyleInfo {
                            id: entry.id,
                            available_colors: entry.available_colors.clone(),
                        })
                        .collect();

                    Ok(hairstyles)
                } else {
                    Err(
                        "Hair styles are still loading. Please wait a moment and try again."
                            .to_string(),
                    )
                };

                let _ = response_tx.send(result);
            }
            TauriEvent::UpdateCreationPreview {
                gender,
                hair_style,
                hair_color,
            } => {
                update_preview_events.write(UpdateCharacterPreviewEvent {
                    gender: Gender::from(gender),
                    hair_style,
                    hair_color,
                });
            }
            TauriEvent::EnterCharacterCreation => {
                open_creation_events.write(OpenCharacterCreationEvent { slot: 0 });
            }
            TauriEvent::ExitCharacterCreation => {
                close_creation_events.write(CloseCharacterCreationEvent);
            }
        }
    }
}
