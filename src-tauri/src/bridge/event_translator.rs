use super::{HairstyleInfo, TauriEvent, TauriEventReceiver};
use crate::bridge::pending_senders::PendingSenders;
use bevy::prelude::*;
use bevy::input::ButtonInput;
use game_engine::domain::character::{
    catalog::HeadStyleCatalog, CharacterCreationForm, CreateCharacterRequestEvent,
    DeleteCharacterRequestEvent, Gender, JobClass, RequestCharacterListEvent,
    SelectCharacterEvent,
};
use game_engine::infrastructure::networking::session::UserSession;
use game_engine::presentation::ui::events::{LoginAttemptEvent, ServerSelectedEvent};
use secrecy::SecretString;

/// Convert JavaScript KeyboardEvent.code to Bevy KeyCode
fn js_code_to_bevy_keycode(code: &str) -> Option<KeyCode> {
    match code {
        // Letters
        "KeyA" => Some(KeyCode::KeyA),
        "KeyB" => Some(KeyCode::KeyB),
        "KeyC" => Some(KeyCode::KeyC),
        "KeyD" => Some(KeyCode::KeyD),
        "KeyE" => Some(KeyCode::KeyE),
        "KeyF" => Some(KeyCode::KeyF),
        "KeyG" => Some(KeyCode::KeyG),
        "KeyH" => Some(KeyCode::KeyH),
        "KeyI" => Some(KeyCode::KeyI),
        "KeyJ" => Some(KeyCode::KeyJ),
        "KeyK" => Some(KeyCode::KeyK),
        "KeyL" => Some(KeyCode::KeyL),
        "KeyM" => Some(KeyCode::KeyM),
        "KeyN" => Some(KeyCode::KeyN),
        "KeyO" => Some(KeyCode::KeyO),
        "KeyP" => Some(KeyCode::KeyP),
        "KeyQ" => Some(KeyCode::KeyQ),
        "KeyR" => Some(KeyCode::KeyR),
        "KeyS" => Some(KeyCode::KeyS),
        "KeyT" => Some(KeyCode::KeyT),
        "KeyU" => Some(KeyCode::KeyU),
        "KeyV" => Some(KeyCode::KeyV),
        "KeyW" => Some(KeyCode::KeyW),
        "KeyX" => Some(KeyCode::KeyX),
        "KeyY" => Some(KeyCode::KeyY),
        "KeyZ" => Some(KeyCode::KeyZ),
        // Arrow keys
        "ArrowUp" => Some(KeyCode::ArrowUp),
        "ArrowDown" => Some(KeyCode::ArrowDown),
        "ArrowLeft" => Some(KeyCode::ArrowLeft),
        "ArrowRight" => Some(KeyCode::ArrowRight),
        // Numbers
        "Digit1" => Some(KeyCode::Digit1),
        "Digit2" => Some(KeyCode::Digit2),
        "Digit3" => Some(KeyCode::Digit3),
        "Digit4" => Some(KeyCode::Digit4),
        "Digit5" => Some(KeyCode::Digit5),
        "Digit6" => Some(KeyCode::Digit6),
        "Digit7" => Some(KeyCode::Digit7),
        "Digit8" => Some(KeyCode::Digit8),
        "Digit9" => Some(KeyCode::Digit9),
        "Digit0" => Some(KeyCode::Digit0),
        // Special keys
        "Space" => Some(KeyCode::Space),
        "Enter" => Some(KeyCode::Enter),
        "Escape" => Some(KeyCode::Escape),
        "Tab" => Some(KeyCode::Tab),
        "Backspace" => Some(KeyCode::Backspace),
        "ShiftLeft" | "ShiftRight" => Some(KeyCode::ShiftLeft),
        "ControlLeft" | "ControlRight" => Some(KeyCode::ControlLeft),
        "AltLeft" | "AltRight" => Some(KeyCode::AltLeft),
        _ => None,
    }
}

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
    mut pending: ResMut<PendingSenders>,
    mut keyboard_input: ResMut<ButtonInput<KeyCode>>,
    session: Option<Res<UserSession>>,
    catalog: Option<Res<HeadStyleCatalog>>,
) {
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
            TauriEvent::KeyboardInput { code, pressed } => {
                if let Some(keycode) = js_code_to_bevy_keycode(&code) {
                    if pressed {
                        keyboard_input.press(keycode);
                    } else {
                        keyboard_input.release(keycode);
                    }
                }
            }
        }
    }
}
