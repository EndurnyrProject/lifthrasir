use bevy::prelude::*;

use super::app_bridge::{TauriEventReceiver, TauriIncomingEvent};
use super::correlation::{CharacterCorrelation, LoginCorrelation, ServerCorrelation};
use super::event_writers::TauriEventWriters;
use super::events::*;
use super::pending_senders::PendingSenders;

/// System that demultiplexes TauriIncomingEvents from the flume channel into typed Bevy events
pub fn demux_tauri_events(
    receiver: Res<TauriEventReceiver>,
    mut pending: ResMut<PendingSenders>,
    mut login_correlation: ResMut<LoginCorrelation>,
    mut char_correlation: ResMut<CharacterCorrelation>,
    mut server_correlation: ResMut<ServerCorrelation>,
    mut writers: TauriEventWriters,
) {
    for event in receiver.0.try_iter() {
        match event {
            TauriIncomingEvent::Login {
                request_id,
                username,
                password,
                response_tx,
            } => {
                // Store correlation before dispatching event
                login_correlation.insert(username.clone(), request_id);

                crate::dispatch_tauri_event!(
                    pending: (pending.logins, request_id, response_tx),
                    event: LoginRequestedEvent {
                        request_id,
                        username,
                        password,
                    },
                    writer: writers.login
                );
            }
            TauriIncomingEvent::SelectServer {
                request_id,
                server_index,
                response_tx,
            } => {
                // Store correlation before dispatching event
                server_correlation.insert(server_index, request_id);

                crate::dispatch_tauri_event!(
                    pending: (pending.servers, request_id, response_tx),
                    event: ServerSelectionRequestedEvent {
                        request_id,
                        server_index,
                    },
                    writer: writers.server_selection
                );
            }
            TauriIncomingEvent::GetCharacterList {
                request_id,
                response_tx,
            } => {
                crate::dispatch_tauri_event!(
                    pending: (pending.char_lists, request_id, response_tx),
                    event: GetCharacterListRequestedEvent { request_id },
                    writer: writers.char_list
                );
            }
            TauriIncomingEvent::SelectCharacter {
                request_id,
                slot,
                response_tx,
            } => {
                // Store correlation before dispatching event
                char_correlation.insert_slot(slot, request_id);

                crate::dispatch_tauri_event!(
                    pending: (pending.char_selections, request_id, response_tx),
                    event: SelectCharacterRequestedEvent { request_id, slot },
                    writer: writers.char_select
                );
            }
            TauriIncomingEvent::CreateCharacter {
                request_id,
                name,
                slot,
                hair_style,
                hair_color,
                sex,
                response_tx,
            } => {
                // Store correlation before dispatching event
                char_correlation.insert_slot(slot, request_id);

                crate::dispatch_tauri_event!(
                    pending: (pending.char_creations, request_id, response_tx),
                    event: CreateCharacterRequestedEvent {
                        request_id,
                        name,
                        slot,
                        hair_style,
                        hair_color,
                        sex,
                    },
                    writer: writers.char_create
                );
            }
            TauriIncomingEvent::DeleteCharacter {
                request_id,
                char_id,
                response_tx,
            } => {
                // Store correlation before dispatching event
                char_correlation.insert_char_id(char_id, request_id);

                crate::dispatch_tauri_event!(
                    pending: (pending.char_deletions, request_id, response_tx),
                    event: DeleteCharacterRequestedEvent {
                        request_id,
                        char_id,
                    },
                    writer: writers.char_delete
                );
            }
            TauriIncomingEvent::GetHairstyles {
                request_id,
                gender,
                response_tx,
            } => {
                crate::dispatch_tauri_event!(
                    pending: (pending.hairstyles, request_id, response_tx),
                    event: GetHairstylesRequestedEvent { request_id, gender },
                    writer: writers.hairstyles
                );
            }
            TauriIncomingEvent::KeyboardInput { code, pressed } => {
                crate::dispatch_tauri_event!(
                    event: KeyboardInputEvent { code, pressed },
                    writer: writers.keyboard
                );
            }
            TauriIncomingEvent::MousePosition { x, y } => {
                crate::dispatch_tauri_event!(
                    event: MousePositionEvent { x, y },
                    writer: writers.mouse
                );
            }
        }
    }
}
