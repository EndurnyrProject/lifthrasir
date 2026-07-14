use super::resource::PartyState;
use bevy::prelude::*;
use net_contract::events::{PartyDisbanded, PartyInfoReceived, PartyMemberUpdated};

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

pub fn apply_party_member_updates(
    mut updates: MessageReader<PartyMemberUpdated>,
    mut state: ResMut<PartyState>,
) {
    for update in updates.read() {
        if state.party_id == 0 || state.party_id != update.party_id {
            continue;
        }

        let Some(index) = state
            .members
            .iter()
            .position(|member| member.char_id == update.member.char_id)
        else {
            continue;
        };

        if state.members[index] == update.member {
            continue;
        }

        state.members[index] = update.member.clone();
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
    use net_contract::events::{PartyDisbanded, PartyInfoReceived, PartyMemberUpdated};

    #[derive(Resource, Default)]
    struct PartyChangeCount(u32);

    fn count_party_changes(state: Res<PartyState>, mut count: ResMut<PartyChangeCount>) {
        if state.is_changed() {
            count.0 += 1;
        }
    }

    fn member(char_id: u32) -> PartyMemberInfo {
        PartyMemberInfo {
            char_id,
            name: "Test".into(),
            base_level: 1,
            online: true,
            map: "prontera".into(),
            job_id: 0,
            hp: 0,
            max_hp: 0,
            sp: 0,
            max_sp: 0,
            ap: 0,
            max_ap: 0,
        }
    }

    fn app_with_party() -> App {
        let mut app = App::new();
        app.add_message::<PartyInfoReceived>();
        app.add_message::<PartyDisbanded>();
        app.add_message::<PartyMemberUpdated>();
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
    fn matching_member_update_replaces_member_in_place() {
        let mut app = app_with_party();
        let leader = member(42);
        let original = member(7);
        let unrelated = member(9);

        app.world_mut().write_message(PartyInfoReceived {
            party_id: 5,
            name: "Aesir".into(),
            leader_char_id: 42,
            exp_share: true,
            members: vec![leader.clone(), original, unrelated.clone()],
        });
        app.update();

        let mut updated = member(7);
        updated.name = "Updated".into();
        updated.hp = 50;
        updated.max_hp = 100;
        app.world_mut().write_message(PartyMemberUpdated {
            party_id: 5,
            member: updated.clone(),
        });
        app.update();

        let state = app.world().resource::<PartyState>();
        assert_eq!(state.party_id, 5);
        assert_eq!(state.name, "Aesir");
        assert_eq!(state.leader_char_id, 42);
        assert!(state.exp_share);
        assert_eq!(state.members, vec![leader, updated, unrelated]);
    }

    #[test]
    fn wrong_party_member_update_is_ignored() {
        let mut app = app_with_party();
        let original = member(7);

        app.world_mut().write_message(PartyInfoReceived {
            party_id: 5,
            name: "Aesir".into(),
            leader_char_id: 7,
            exp_share: false,
            members: vec![original.clone()],
        });
        app.update();

        let mut updated = member(7);
        updated.hp = 50;
        app.world_mut().write_message(PartyMemberUpdated {
            party_id: 6,
            member: updated,
        });
        app.update();

        let state = app.world().resource::<PartyState>();
        assert_eq!(state.party_id, 5);
        assert_eq!(state.name, "Aesir");
        assert_eq!(state.leader_char_id, 7);
        assert!(!state.exp_share);
        assert_eq!(state.members, vec![original]);
    }

    #[test]
    fn unknown_member_update_is_ignored() {
        let mut app = app_with_party();
        let original = member(7);

        app.world_mut().write_message(PartyInfoReceived {
            party_id: 5,
            name: "Aesir".into(),
            leader_char_id: 7,
            exp_share: false,
            members: vec![original.clone()],
        });
        app.update();

        app.world_mut().write_message(PartyMemberUpdated {
            party_id: 5,
            member: member(8),
        });
        app.update();

        let state = app.world().resource::<PartyState>();
        assert_eq!(state.party_id, 5);
        assert_eq!(state.name, "Aesir");
        assert_eq!(state.leader_char_id, 7);
        assert!(!state.exp_share);
        assert_eq!(state.members, vec![original]);
    }

    #[test]
    fn identical_member_update_does_not_mark_party_state_changed() {
        let mut app = app_with_party();
        app.init_resource::<PartyChangeCount>()
            .add_systems(Update, count_party_changes.after(super::clear_on_disband));

        app.world_mut().write_message(PartyInfoReceived {
            party_id: 5,
            name: "Aesir".into(),
            leader_char_id: 7,
            exp_share: false,
            members: vec![member(7)],
        });
        app.update();
        app.world_mut().resource_mut::<PartyChangeCount>().0 = 0;

        app.world_mut().write_message(PartyMemberUpdated {
            party_id: 5,
            member: member(7),
        });
        app.update();

        assert_eq!(app.world().resource::<PartyChangeCount>().0, 0);
    }

    #[test]
    fn member_update_without_active_party_is_ignored() {
        let mut app = app_with_party();

        app.world_mut().write_message(PartyMemberUpdated {
            party_id: 0,
            member: member(7),
        });
        app.update();

        let state = app.world().resource::<PartyState>();
        assert_eq!(state.party_id, 0);
        assert!(state.members.is_empty());
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

        app.world_mut().write_message(PartyMemberUpdated {
            party_id: 5,
            member: member(42),
        });
        app.update();

        let state = app.world().resource::<PartyState>();
        assert!(!state.in_party());
        assert!(state.members.is_empty());
    }

    #[test]
    fn same_frame_party_info_update_and_disband_ends_empty() {
        let mut app = app_with_party();
        let mut updated = member(42);
        updated.hp = 50;

        app.world_mut().write_message(PartyInfoReceived {
            party_id: 5,
            name: "Aesir".into(),
            leader_char_id: 42,
            exp_share: true,
            members: vec![member(42)],
        });
        app.world_mut().write_message(PartyMemberUpdated {
            party_id: 5,
            member: updated,
        });
        app.world_mut().write_message(PartyDisbanded {
            party_id: 5,
            reason: "left".into(),
        });
        app.update();

        let state = app.world().resource::<PartyState>();
        assert_eq!(state.party_id, 0);
        assert!(state.name.is_empty());
        assert_eq!(state.leader_char_id, 0);
        assert!(!state.exp_share);
        assert!(state.members.is_empty());
    }

    #[test]
    fn reset_party_on_character_select_clears_state() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.add_message::<PartyInfoReceived>();
        app.add_message::<PartyDisbanded>();
        app.add_message::<PartyMemberUpdated>();
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
        app.world_mut().write_message(PartyMemberUpdated {
            party_id: 5,
            member: member(42),
        });
        app.update();

        let state = app.world().resource::<PartyState>();
        assert!(!state.in_party());
        assert!(state.members.is_empty());
    }
}
