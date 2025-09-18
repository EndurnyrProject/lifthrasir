use crate::core::state::{CharacterScreenState, GameState};
use crate::domain::character::*;
use crate::presentation::ui::screens::character_selection::creation::CharacterCreationResource;
use crate::presentation::ui::screens::character_selection::list::CharacterSelectionResource;
use bevy::prelude::*;

pub fn handle_character_card_click(
    interaction_query: Query<(&Interaction, &CharacterCard), Changed<Interaction>>,
    mut selection: ResMut<CharacterSelectionResource>,
    mut select_events: EventWriter<SelectCharacterEvent>,
) {
    for (interaction, card) in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            if let Some(character) = &card.character {
                selection.selected_character = Some(character.clone());
                selection.selected_slot = Some(card.slot);
                select_events.write(SelectCharacterEvent { slot: card.slot });
                info!(
                    "Selected character: {} in slot {}",
                    character.name, card.slot
                );
            } else {
                // Empty slot clicked - could trigger character creation
                info!("Empty slot {} clicked", card.slot);
            }
        }
    }
}

pub fn handle_select_button_click(
    _enter_buttons: Query<&EnterGameButton>,
    selection: Res<CharacterSelectionResource>,
    mut enter_events: EventWriter<EnterGameRequestEvent>,
) {
    if let Some(character) = &selection.selected_character {
        let _ = enter_events.write(EnterGameRequestEvent {
            character_id: character.char_id,
        });
    }
}

pub fn handle_delete_button_click(
    delete_buttons: Query<&DeleteCharacterButton>,
    mut delete_events: EventWriter<DeleteCharacterRequestEvent>,
    mut char_state: ResMut<NextState<CharacterScreenState>>,
) {
    // TODO: Implement click detection for delete button using observers
    for button in delete_buttons.iter() {
        let _ = (&mut delete_events, &mut char_state, button);
    }
}

pub fn handle_back_button_click(
    _back_buttons: Query<&BackToServerSelectionButton>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    // TODO: Implement click detection for back button using observers
    // Will be triggered by button click
    let _ = &mut game_state;
}

pub fn handle_character_hover(
    character_cards: Query<&CharacterCard>,
    mut selection: ResMut<CharacterSelectionResource>,
    mut hover_event: EventWriter<CharacterHoverEvent>,
) {
    // TODO: Implement hover detection for character cards using observers
    for card in character_cards.iter() {
        let _ = (&mut selection, &mut hover_event, card);
    }
}

pub fn handle_creation_form_cancel(
    _cancel_buttons: Query<&CancelCharacterCreationButton>,
    mut creation_resource: ResMut<CharacterCreationResource>,
    mut close_events: EventWriter<CloseCharacterCreationEvent>,
    mut char_state: ResMut<NextState<CharacterScreenState>>,
) {
    // TODO: Implement click detection for cancel button using observers
    creation_resource.reset();
    close_events.write(CloseCharacterCreationEvent);
    char_state.set(CharacterScreenState::CharacterList);
}

// Handle the OpenCharacterCreationEvent to transition states
pub fn handle_open_character_creation(
    mut events: EventReader<OpenCharacterCreationEvent>,
    mut char_state: ResMut<NextState<CharacterScreenState>>,
    current_state: Res<State<CharacterScreenState>>,
) {
    for event in events.read() {
        info!(
            "handle_open_character_creation: Processing OpenCharacterCreationEvent for slot {} - Current state: {:?}",
            event.slot,
            current_state.get()
        );
        info!("handle_open_character_creation: Setting state to CharacterCreation");
        char_state.set(CharacterScreenState::CharacterCreation);
    }
}

// Handle character creation back button clicks
pub fn handle_character_creation_back_button(
    interaction_query: Query<
        &Interaction,
        (Changed<Interaction>, With<CharacterCreationBackButton>),
    >,
    mut creation_resource: ResMut<CharacterCreationResource>,
    mut close_events: EventWriter<CloseCharacterCreationEvent>,
    mut char_state: ResMut<NextState<CharacterScreenState>>,
    current_state: Res<State<CharacterScreenState>>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            info!(
                "Character creation back button clicked - Current state: {:?}",
                current_state.get()
            );
            creation_resource.reset();
            close_events.write(CloseCharacterCreationEvent);
            char_state.set(CharacterScreenState::CharacterList);
        }
    }
}

// Keyboard shortcuts
pub fn handle_keyboard_shortcuts(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    selection: Res<CharacterSelectionResource>,
    mut select_events: EventWriter<SelectCharacterEvent>,
    mut enter_events: EventWriter<EnterGameRequestEvent>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    // Enter key - select character
    if keyboard_input.just_pressed(KeyCode::Enter) {
        if let Some(character) = &selection.selected_character {
            enter_events.write(EnterGameRequestEvent {
                character_id: character.char_id,
            });
        }
    }

    // ESC key - go back
    if keyboard_input.just_pressed(KeyCode::Escape) {
        game_state.set(GameState::ServerSelection);
    }

    // Number keys 1-9 - quick select character
    for (key, slot) in [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
        (KeyCode::Digit5, 4),
        (KeyCode::Digit6, 5),
        (KeyCode::Digit7, 6),
        (KeyCode::Digit8, 7),
        (KeyCode::Digit9, 8),
    ] {
        if keyboard_input.just_pressed(key) {
            select_events.write(SelectCharacterEvent { slot });
        }
    }
}
