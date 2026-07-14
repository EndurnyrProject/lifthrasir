use bevy::prelude::*;
use net_contract::dto::{GuildInfo, GuildMemberInfo, GuildPositionInfo};

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct GuildState {
    info: Option<GuildInfo>,
}

impl GuildState {
    pub fn in_guild(&self) -> bool {
        self.info.is_some()
    }

    pub fn info(&self) -> Option<&GuildInfo> {
        self.info.as_ref()
    }

    pub fn is_master(&self, char_id: u32) -> bool {
        char_id != 0
            && self
                .info
                .as_ref()
                .is_some_and(|info| info.master_char_id == char_id)
    }

    pub fn member(&self, char_id: u32) -> Option<&GuildMemberInfo> {
        self.info
            .as_ref()?
            .members
            .iter()
            .find(|member| member.char_id == char_id)
    }

    pub fn position(&self, index: u32) -> Option<&GuildPositionInfo> {
        self.info
            .as_ref()?
            .positions
            .iter()
            .find(|position| position.index == index)
    }

    pub fn can_invite(&self, char_id: u32) -> bool {
        self.member(char_id)
            .and_then(|member| self.position(member.position_index))
            .is_some_and(|position| position.can_invite)
    }

    pub fn can_expel(&self, char_id: u32) -> bool {
        self.member(char_id)
            .and_then(|member| self.position(member.position_index))
            .is_some_and(|position| position.can_expel)
    }

    pub(crate) fn replace(&mut self, info: GuildInfo) {
        self.info = Some(info);
    }

    pub(crate) fn clear(&mut self) {
        self.info = None;
    }

    pub(crate) fn update_member(&mut self, member: GuildMemberInfo) {
        let Some(current) = self.info.as_mut().and_then(|info| {
            info.members
                .iter_mut()
                .find(|item| item.char_id == member.char_id)
        }) else {
            return;
        };
        if *current != member {
            *current = member;
        }
    }

    pub(crate) fn update_emblem(&mut self, emblem_id: u32) {
        let Some(info) = self.info.as_mut() else {
            return;
        };
        if info.emblem_id != emblem_id {
            info.emblem_id = emblem_id;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::{GuildMemberInfo, GuildPositionInfo};

    fn info() -> GuildInfo {
        GuildInfo {
            guild_id: 7,
            name: "Vikings".into(),
            master_char_id: 42,
            emblem_id: 3,
            notice_subject: "Welcome".into(),
            notice_body: "Be kind".into(),
            positions: vec![
                GuildPositionInfo {
                    index: 0,
                    name: "Master".into(),
                    can_invite: true,
                    can_expel: true,
                    can_storage: false,
                    tax: 0,
                },
                GuildPositionInfo {
                    index: 1,
                    name: "Member".into(),
                    can_invite: false,
                    can_expel: false,
                    can_storage: false,
                    tax: 0,
                },
            ],
            members: vec![
                GuildMemberInfo {
                    char_id: 42,
                    name: "Odin".into(),
                    job_id: 1,
                    base_level: 99,
                    online: true,
                    map: "prontera".into(),
                    position_index: 0,
                    hp: 100,
                    max_hp: 100,
                    sp: 50,
                    max_sp: 50,
                    ap: 0,
                    max_ap: 0,
                },
                GuildMemberInfo {
                    char_id: 43,
                    name: "Thor".into(),
                    job_id: 2,
                    base_level: 80,
                    online: true,
                    map: "geffen".into(),
                    position_index: 1,
                    hp: 80,
                    max_hp: 100,
                    sp: 40,
                    max_sp: 50,
                    ap: 0,
                    max_ap: 0,
                },
            ],
        }
    }

    #[test]
    fn default_state_is_unguilded() {
        let state = GuildState::default();

        assert!(!state.in_guild());
        assert!(state.info().is_none());
    }

    #[test]
    fn queries_follow_the_authoritative_snapshot() {
        let mut state = GuildState::default();
        state.replace(info());

        assert!(state.in_guild());
        assert!(state.is_master(42));
        assert!(!state.is_master(43));
        assert_eq!(
            state.member(43).map(|member| member.name.as_str()),
            Some("Thor")
        );
        assert_eq!(
            state.position(1).map(|position| position.name.as_str()),
            Some("Member")
        );
        assert!(state.can_invite(42));
        assert!(state.can_expel(42));
        assert!(!state.can_invite(43));
        assert!(!state.can_expel(43));
        assert!(!state.can_invite(999));
        assert!(!state.can_expel(999));
    }
}
