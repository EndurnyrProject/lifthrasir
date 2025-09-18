use crate::core::state::CharacterScreenState;
use bevy::prelude::*;

pub fn log_state_changes(
    current_state: Res<State<CharacterScreenState>>,
    mut last_state: Local<Option<CharacterScreenState>>,
) {
    let current = current_state.get().clone();
    if last_state.as_ref() != Some(&current) {
        info!("CharacterScreenState changed: {:?} -> {:?}", *last_state, current);
        *last_state = Some(current);
    }
}

pub fn log_game_state_changes(
    current_state: Res<State<crate::core::state::GameState>>,
    mut last_state: Local<Option<crate::core::state::GameState>>,
) {
    let current = current_state.get().clone();
    if last_state.as_ref() != Some(&current) {
        info!("GameState changed: {:?} -> {:?}", *last_state, current);
        *last_state = Some(current);
    }
}
