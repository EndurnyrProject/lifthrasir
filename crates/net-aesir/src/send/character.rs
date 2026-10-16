use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{
    CreateCharacter, DeleteCharacter, RefreshCharacterList, SelectCharacter,
};

use crate::channels::CONTROL;
use crate::character::{CharPhase, QuicCharState};
use crate::envelope::Body;
use crate::proto::aesir::net::{CharListRefresh, CreateChar, DeleteCharRequest, SelectChar};

fn select_char_body(c: &SelectCharacter) -> Body {
    Body::SelectChar(SelectChar { slot: c.slot })
}

/// Maps a flattened creation command onto the proto `CreateChar` request.
fn create_char_body(c: &CreateCharacter) -> Body {
    Body::CreateChar(CreateChar {
        name: c.name.clone(),
        slot: c.slot,
        hair_color: c.hair_color,
        hair_style: c.hair_style,
        starting_job: c.starting_job,
        sex: c.sex,
    })
}

fn delete_char_body(c: &DeleteCharacter) -> Body {
    Body::DeleteCharRequest(DeleteCharRequest { char_id: c.char_id })
}

fn refresh_body(_: &RefreshCharacterList) -> Body {
    Body::CharListRefresh(CharListRefresh {})
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_select_character(
    mut events: MessageReader<SelectCharacter>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
) {
    for ev in events.read() {
        if state.phase != CharPhase::Ready {
            continue;
        }
        if let Err(e) = state
            .conn
            .send(client.connection_mut(), CONTROL, select_char_body(ev))
        {
            error!("failed to send SelectChar: {e}");
            continue;
        }
        state.phase = CharPhase::Selecting;
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_create_character(
    mut events: MessageReader<CreateCharacter>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
) {
    for ev in events.read() {
        if state.phase != CharPhase::Ready {
            continue;
        }
        if let Err(e) = state
            .conn
            .send(client.connection_mut(), CONTROL, create_char_body(ev))
        {
            error!("failed to send CreateChar: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_delete_character(
    mut events: MessageReader<DeleteCharacter>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
) {
    for ev in events.read() {
        if state.phase != CharPhase::Ready {
            continue;
        }
        if let Err(e) = state
            .conn
            .send(client.connection_mut(), CONTROL, delete_char_body(ev))
        {
            error!("failed to send DeleteCharRequest: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_refresh_character_list(
    mut events: MessageReader<RefreshCharacterList>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
) {
    for ev in events.read() {
        if state.phase != CharPhase::Ready {
            continue;
        }
        if let Err(e) = state
            .conn
            .send(client.connection_mut(), CONTROL, refresh_body(ev))
        {
            error!("failed to send CharListRefresh: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_char_body_maps_field_for_field() {
        let body = create_char_body(&CreateCharacter {
            name: "Hero".into(),
            slot: 2,
            hair_color: 3,
            hair_style: 7,
            starting_job: 0,
            sex: 1,
        });
        match body {
            Body::CreateChar(req) => {
                assert_eq!(req.name, "Hero");
                assert_eq!(req.slot, 2);
                assert_eq!(req.hair_style, 7);
                assert_eq!(req.hair_color, 3);
                assert_eq!(req.starting_job, 0);
                assert_eq!(req.sex, 1);
            }
            other => panic!("expected Body::CreateChar, got {other:?}"),
        }
    }
}
