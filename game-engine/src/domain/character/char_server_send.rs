use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::QuinnetClient;

use crate::domain::character::events::{
    CreateCharacterRequestEvent, DeleteCharacterRequestEvent, RefreshCharacterListEvent,
    SelectCharacterEvent,
};
use crate::domain::character::forms::CharacterCreationForm;
use crate::infrastructure::networking::quic::channels::CONTROL;
use crate::infrastructure::networking::quic::character::{CharPhase, QuicCharState};
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::quic::proto::aesir::net::{
    CharListRefresh, CreateChar, DeleteCharRequest, SelectChar,
};

/// Maps a validated creation form onto the proto `CreateChar` request.
fn form_to_create_char(form: &CharacterCreationForm) -> CreateChar {
    CreateChar {
        name: form.name.clone(),
        slot: form.slot as u32,
        hair_color: form.hair_color as u32,
        hair_style: form.hair_style as u32,
        starting_job: form.starting_job as u32,
        sex: form.sex as u32,
    }
}

/// Sends `SelectChar` for a UI-selected slot while the session is `Ready`.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update
)]
pub fn char_send_select(
    mut events: MessageReader<SelectCharacterEvent>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
) {
    for ev in events.read() {
        if state.phase != CharPhase::Ready {
            continue;
        }
        let body = Body::SelectChar(SelectChar {
            slot: ev.slot as u32,
        });
        if let Err(e) = state.conn.send(client.connection_mut(), CONTROL, body) {
            error!("failed to send SelectChar: {e}");
            continue;
        }
        state.phase = CharPhase::Selecting;
    }
}

/// Sends `CreateChar` for a UI creation request while the session is `Ready`.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update
)]
pub fn char_send_create(
    mut events: MessageReader<CreateCharacterRequestEvent>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
) {
    for ev in events.read() {
        if state.phase != CharPhase::Ready {
            continue;
        }
        if let Err(e) = ev.form.validate() {
            warn!("rejecting invalid character creation form: {e}");
            continue;
        }
        let body = Body::CreateChar(form_to_create_char(&ev.form));
        if let Err(e) = state.conn.send(client.connection_mut(), CONTROL, body) {
            error!("failed to send CreateChar: {e}");
        }
    }
}

/// Sends `DeleteCharRequest` for a UI deletion request while the session is `Ready`.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update
)]
pub fn char_send_delete(
    mut events: MessageReader<DeleteCharacterRequestEvent>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
) {
    for ev in events.read() {
        if state.phase != CharPhase::Ready {
            continue;
        }
        let body = Body::DeleteCharRequest(DeleteCharRequest {
            char_id: ev.character_id,
        });
        if let Err(e) = state.conn.send(client.connection_mut(), CONTROL, body) {
            error!("failed to send DeleteCharRequest: {e}");
        }
    }
}

/// Sends `CharListRefresh` for a UI refresh request while the session is `Ready`.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update
)]
pub fn char_send_refresh(
    mut events: MessageReader<RefreshCharacterListEvent>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
) {
    for _ in events.read() {
        if state.phase != CharPhase::Ready {
            continue;
        }
        let body = Body::CharListRefresh(CharListRefresh {});
        if let Err(e) = state.conn.send(client.connection_mut(), CONTROL, body) {
            error!("failed to send CharListRefresh: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_to_create_char_maps_field_for_field() {
        use crate::domain::entities::character::components::Gender;

        let form = CharacterCreationForm {
            name: "Hero".into(),
            slot: 2,
            hair_style: 7,
            hair_color: 3,
            starting_job: 0,
            sex: Gender::Male,
            ..Default::default()
        };

        let req = form_to_create_char(&form);
        assert_eq!(req.name, "Hero");
        assert_eq!(req.slot, 2);
        assert_eq!(req.hair_style, 7);
        assert_eq!(req.hair_color, 3);
        assert_eq!(req.starting_job, 0);
        assert_eq!(req.sex, Gender::Male as u32);

        let female_form = CharacterCreationForm {
            sex: Gender::Female,
            ..Default::default()
        };
        assert_eq!(form_to_create_char(&female_form).sex, 0u32);
    }
}
