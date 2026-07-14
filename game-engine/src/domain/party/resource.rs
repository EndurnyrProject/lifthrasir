use bevy::prelude::*;
use net_contract::dto::PartyMemberInfo;

#[derive(Resource, Default)]
pub struct PartyState {
    pub party_id: u32,
    pub name: String,
    pub leader_char_id: u32,
    pub exp_share: bool,
    pub members: Vec<PartyMemberInfo>,
}

impl PartyState {
    pub fn in_party(&self) -> bool {
        self.party_id != 0
    }

    pub fn is_leader(&self, char_id: u32) -> bool {
        self.in_party() && self.leader_char_id == char_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn default_state_is_not_in_party() {
        let state = PartyState::default();

        assert!(!state.in_party());
    }

    #[test]
    fn in_party_true_when_party_id_nonzero() {
        let state = PartyState {
            party_id: 5,
            ..Default::default()
        };

        assert!(state.in_party());
    }

    #[test]
    fn is_leader_true_only_for_leader_char_id_while_in_party() {
        let state = PartyState {
            party_id: 5,
            leader_char_id: 42,
            members: vec![member(42), member(7)],
            ..Default::default()
        };

        assert!(state.is_leader(42));
        assert!(!state.is_leader(7));
    }

    #[test]
    fn is_leader_false_when_not_in_party() {
        let state = PartyState {
            party_id: 0,
            leader_char_id: 42,
            ..Default::default()
        };

        assert!(!state.is_leader(42));
    }
}
