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

    /// The position `char_id` holds, when they are a member.
    pub fn position_of(&self, char_id: u32) -> Option<&GuildPositionInfo> {
        self.position(self.member(char_id)?.position_index)
    }

    /// Whether `char_id` may do what `flag` gates. The master always may, as on
    /// the server; everyone else needs the flag on their position.
    fn permits(&self, char_id: u32, flag: fn(&GuildPositionInfo) -> bool) -> bool {
        self.is_master(char_id) || self.position_of(char_id).is_some_and(flag)
    }

    pub fn can_invite(&self, char_id: u32) -> bool {
        self.permits(char_id, |position| position.can_invite)
    }

    pub fn can_expel(&self, char_id: u32) -> bool {
        self.permits(char_id, |position| position.can_expel)
    }

    pub fn can_notice(&self, char_id: u32) -> bool {
        self.permits(char_id, |position| position.can_notice)
    }

    pub fn can_relations(&self, char_id: u32) -> bool {
        self.permits(char_id, |position| position.can_relations)
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
    use net_contract::dto::{
        GuildMemberInfo, GuildPositionInfo, GuildRelationInfo, GuildRelationKind, GuildSkillInfo,
    };

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
                    ..Default::default()
                },
                GuildPositionInfo {
                    index: 1,
                    name: "Member".into(),
                    can_invite: false,
                    can_expel: false,
                    can_storage: false,
                    tax: 0,
                    ..Default::default()
                },
                GuildPositionInfo {
                    index: 2,
                    name: "Scribe".into(),
                    can_notice: true,
                    can_relations: true,
                    ..Default::default()
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
                    ..Default::default()
                },
                GuildMemberInfo {
                    char_id: 44,
                    name: "Bragi".into(),
                    position_index: 2,
                    ..Default::default()
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
                    ..Default::default()
                },
            ],
            level: 1,
            exp: 0,
            next_exp: 0,
            skill_points: 0,
            skills: vec![],
            relations: vec![],
            ..Default::default()
        }
    }

    #[test]
    fn replace_preserves_the_complete_authoritative_snapshot() {
        let mut expected = info();
        expected.level = 17;
        expected.exp = u32::MAX as u64 + 99;
        expected.next_exp = u32::MAX as u64 + 1_000;
        expected.skill_points = 4;
        expected.skills = vec![GuildSkillInfo {
            skill_id: 10000,
            level: 2,
            max_level: 5,
        }];
        expected.relations = vec![GuildRelationInfo {
            guild_id: 8,
            name: "Allies".into(),
            kind: GuildRelationKind::Ally,
            ..Default::default()
        }];
        let mut state = GuildState::default();

        state.replace(expected.clone());

        assert_eq!(state.info(), Some(&expected));
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

    #[test]
    fn notice_and_relations_follow_the_position_and_the_master_always_may() {
        let mut state = GuildState::default();
        state.replace(info());

        // The master's own position carries neither flag.
        assert!(state.can_notice(42));
        assert!(state.can_relations(42));
        assert!(state.can_notice(44));
        assert!(state.can_relations(44));
        assert!(!state.can_notice(43));
        assert!(!state.can_relations(43));
        assert!(!state.can_notice(999));
    }
}
