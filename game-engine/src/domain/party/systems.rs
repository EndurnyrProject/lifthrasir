use super::resource::PartyState;
use bevy::prelude::*;
use net_contract::events::{PartyDisbanded, PartyInfoReceived};

pub fn apply_party_info(
    mut received: MessageReader<PartyInfoReceived>,
    mut state: ResMut<PartyState>,
) {
    for info in received.read() {
        state.party_id = info.party_id;
        state.name = info.name.clone();
        state.leader_char_id = info.leader_char_id;
        state.exp_share = info.exp_share;
        state.members = info.members.clone();
    }
}

pub fn clear_on_disband(
    mut disbanded: MessageReader<PartyDisbanded>,
    mut state: ResMut<PartyState>,
) {
    for _ in disbanded.read() {
        *state = PartyState::default();
    }
}

pub fn reset_party_on_character_select(mut state: ResMut<PartyState>) {
    *state = PartyState::default();
}

#[cfg(test)]
mod tests {
    use crate::core::state::GameState;
    use crate::domain::party::{PartyPlugin, PartyState};
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;
    use net_contract::dto::PartyMemberInfo;
    use net_contract::events::{PartyDisbanded, PartyInfoReceived};

    fn member(char_id: u32) -> PartyMemberInfo {
        PartyMemberInfo {
            char_id,
            name: "Test".into(),
            base_level: 1,
            online: true,
            map: "prontera".into(),
        }
    }

    fn app_with_party() -> App {
        let mut app = App::new();
        app.add_message::<PartyInfoReceived>();
        app.add_message::<PartyDisbanded>();
        app.add_plugins(PartyPlugin);
        app
    }

    #[test]
    fn party_info_received_populates_state() {
        let mut app = app_with_party();

        app.world_mut().write_message(PartyInfoReceived {
            party_id: 5,
            name: "Aesir".into(),
            leader_char_id: 42,
            exp_share: true,
            members: vec![member(42), member(7)],
        });
        app.update();

        let state = app.world().resource::<PartyState>();
        assert!(state.in_party());
        assert_eq!(state.party_id, 5);
        assert_eq!(state.name, "Aesir");
        assert!(state.is_leader(42));
        assert!(!state.is_leader(7));
        assert_eq!(state.members.len(), 2);
    }

    #[test]
    fn party_disbanded_clears_state() {
        let mut app = app_with_party();

        app.world_mut().write_message(PartyInfoReceived {
            party_id: 5,
            name: "Aesir".into(),
            leader_char_id: 42,
            exp_share: true,
            members: vec![member(42)],
        });
        app.update();
        assert!(app.world().resource::<PartyState>().in_party());

        app.world_mut().write_message(PartyDisbanded {
            party_id: 5,
            reason: "left".into(),
        });
        app.update();

        assert!(!app.world().resource::<PartyState>().in_party());
    }

    #[test]
    fn reset_party_on_character_select_clears_state() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.add_message::<PartyInfoReceived>();
        app.add_message::<PartyDisbanded>();
        app.add_plugins(PartyPlugin);

        app.world_mut().write_message(PartyInfoReceived {
            party_id: 5,
            name: "Aesir".into(),
            leader_char_id: 42,
            exp_share: true,
            members: vec![member(42)],
        });
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        assert!(app.world().resource::<PartyState>().in_party());

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::CharacterSelection);
        app.update();

        assert!(!app.world().resource::<PartyState>().in_party());
    }
}
