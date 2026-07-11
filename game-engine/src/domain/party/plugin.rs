use super::resource::PartyState;
use super::systems;
use crate::core::state::GameState;
use bevy::prelude::*;

pub struct PartyPlugin;

impl Plugin for PartyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PartyState>()
            .add_systems(
                Update,
                (systems::apply_party_info, systems::clear_on_disband),
            )
            .add_systems(
                OnEnter(GameState::CharacterSelection),
                systems::reset_party_on_character_select,
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::events::{PartyDisbanded, PartyInfoReceived};

    #[test]
    fn plugin_registers_resource() {
        let mut app = App::new();
        app.add_message::<PartyInfoReceived>();
        app.add_message::<PartyDisbanded>();
        app.add_plugins(PartyPlugin);

        assert!(app.world().contains_resource::<PartyState>());
    }
}
