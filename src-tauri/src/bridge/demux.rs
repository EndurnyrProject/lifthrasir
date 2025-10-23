use bevy::prelude::*;

use super::app_bridge::{TauriEventReceiver, TauriIncomingEvent};
use super::correlation::{
    CharacterCorrelation, LoginCorrelation, PendingCharacterListSenders, PendingHairstyleSenders,
    ServerCorrelation,
};
use super::event_writers::TauriMessageWriters;
use super::events::*;

pub fn demux_tauri_events(
    receiver: Res<TauriEventReceiver>,
    mut login_correlation: ResMut<LoginCorrelation>,
    mut char_correlation: ResMut<CharacterCorrelation>,
    mut server_correlation: ResMut<ServerCorrelation>,
    mut char_list_senders: ResMut<PendingCharacterListSenders>,
    mut hairstyle_senders: ResMut<PendingHairstyleSenders>,
    mut writers: TauriMessageWriters,
) {
    for event in receiver.0.try_iter() {
        match event {
            TauriIncomingEvent::Login {
                username,
                password,
                response_tx,
            } => {
                login_correlation.insert(username.clone(), response_tx);
                writers
                    .login
                    .write(LoginRequestedEvent { username, password });
            }
            TauriIncomingEvent::SelectServer {
                server_index,
                response_tx,
            } => {
                server_correlation.insert(server_index, response_tx);
                writers
                    .server_selection
                    .write(ServerSelectionRequestedEvent { server_index });
            }
            TauriIncomingEvent::GetCharacterList { response_tx } => {
                char_list_senders.push(response_tx);
                writers.char_list.write(GetCharacterListRequestedEvent {});
            }
            TauriIncomingEvent::SelectCharacter { slot, response_tx } => {
                char_correlation.insert_selection(slot, response_tx);
                writers
                    .char_select
                    .write(SelectCharacterRequestedEvent { slot });
            }
            TauriIncomingEvent::CreateCharacter {
                name,
                slot,
                hair_style,
                hair_color,
                sex,
                response_tx,
            } => {
                char_correlation.insert_creation(slot, response_tx);
                writers.char_create.write(CreateCharacterRequestedEvent {
                    name,
                    slot,
                    hair_style,
                    hair_color,
                    sex,
                });
            }
            TauriIncomingEvent::DeleteCharacter {
                char_id,
                response_tx,
            } => {
                char_correlation.insert_deletion(char_id, response_tx);
                writers
                    .char_delete
                    .write(DeleteCharacterRequestedEvent { char_id });
            }
            TauriIncomingEvent::GetHairstyles {
                gender,
                response_tx,
            } => {
                hairstyle_senders.push(response_tx);
                writers
                    .hairstyles
                    .write(GetHairstylesRequestedEvent { gender });
            }
            TauriIncomingEvent::KeyboardInput { code, pressed } => {
                writers.keyboard.write(KeyboardInputEvent { code, pressed });
            }
            TauriIncomingEvent::MousePosition { x, y } => {
                writers.mouse.write(MousePositionEvent { x, y });
            }
            TauriIncomingEvent::MouseClick { x, y } => {
                writers.mouse_click.write(MouseClickEvent { x, y });
            }
        }
    }
}
