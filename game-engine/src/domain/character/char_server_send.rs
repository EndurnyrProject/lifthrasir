use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use net_contract::commands::{
    CreateCharacter, DeleteCharacter, RefreshCharacterList, SelectCharacter,
};

use crate::domain::character::events::{
    CreateCharacterRequestEvent, DeleteCharacterRequestEvent, RefreshCharacterListEvent,
    SelectCharacterEvent,
};
use crate::domain::character::forms::CharacterCreationForm;

/// Flattens a validated creation form into the primitive `CreateCharacter` command.
fn form_to_create_character(form: &CharacterCreationForm) -> CreateCharacter {
    CreateCharacter {
        name: form.name.clone(),
        slot: form.slot as u32,
        hair_color: form.hair_color as u32,
        hair_style: form.hair_style as u32,
        starting_job: form.starting_job as u32,
        sex: form.sex as u32,
    }
}

/// Bridges a UI character selection onto the outbound `SelectCharacter` command.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update
)]
pub fn char_send_select(
    mut events: MessageReader<SelectCharacterEvent>,
    mut commands: MessageWriter<SelectCharacter>,
) {
    for ev in events.read() {
        commands.write(SelectCharacter {
            slot: ev.slot as u32,
        });
    }
}

/// Bridges a UI creation request onto the outbound `CreateCharacter` command.
///
/// Validates and flattens the form before writing; invalid forms are dropped.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update
)]
pub fn char_send_create(
    mut events: MessageReader<CreateCharacterRequestEvent>,
    mut commands: MessageWriter<CreateCharacter>,
) {
    for ev in events.read() {
        if let Err(e) = ev.form.validate() {
            warn!("rejecting invalid character creation form: {e}");
            continue;
        }
        commands.write(form_to_create_character(&ev.form));
    }
}

/// Bridges a UI deletion request onto the outbound `DeleteCharacter` command.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update
)]
pub fn char_send_delete(
    mut events: MessageReader<DeleteCharacterRequestEvent>,
    mut commands: MessageWriter<DeleteCharacter>,
) {
    for ev in events.read() {
        commands.write(DeleteCharacter {
            char_id: ev.character_id,
        });
    }
}

/// Bridges a UI refresh request onto the outbound `RefreshCharacterList` command.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update
)]
pub fn char_send_refresh(
    mut events: MessageReader<RefreshCharacterListEvent>,
    mut commands: MessageWriter<RefreshCharacterList>,
) {
    for _ in events.read() {
        commands.write(RefreshCharacterList);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_to_create_character_maps_field_for_field() {
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

        let cmd = form_to_create_character(&form);
        assert_eq!(cmd.name, "Hero");
        assert_eq!(cmd.slot, 2);
        assert_eq!(cmd.hair_style, 7);
        assert_eq!(cmd.hair_color, 3);
        assert_eq!(cmd.starting_job, 0);
        assert_eq!(cmd.sex, Gender::Male as u32);

        let female_form = CharacterCreationForm {
            sex: Gender::Female,
            ..Default::default()
        };
        assert_eq!(form_to_create_character(&female_form).sex, 0u32);
    }
}
