use game_engine::infrastructure::job::JobSpriteRegistry;
use net_contract::dto::GuildInfo;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MemberRow {
    pub name: String,
    pub position: String,
    pub job: String,
    pub level: u32,
    pub online: bool,
    pub map: String,
    pub hp: (u64, u64),
    pub sp: (u64, u64),
    pub ap: Option<(u32, u32)>,
}

pub(crate) fn project_rows(info: &GuildInfo, jobs: Option<&JobSpriteRegistry>) -> Vec<MemberRow> {
    info.members
        .iter()
        .map(|member| MemberRow {
            name: member.name.clone(),
            position: info
                .positions
                .iter()
                .find(|position| position.index == member.position_index)
                .map(|position| position.name.clone())
                .unwrap_or_else(|| "Unknown Position".to_string()),
            job: jobs
                .and_then(|jobs| jobs.try_display_name(member.job_id))
                .unwrap_or("Unknown Job")
                .to_string(),
            level: member.base_level,
            online: member.online,
            map: member.map.clone(),
            hp: (member.hp, member.max_hp),
            sp: (member.sp, member.max_sp),
            ap: (member.max_ap > 0).then_some((member.ap, member.max_ap)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use net_contract::dto::{GuildInfo, GuildMemberInfo, GuildPositionInfo};

    #[test]
    fn projects_authoritative_online_and_offline_roster_rows() {
        let info = GuildInfo {
            guild_id: 7,
            name: "Vikings".into(),
            master_char_id: 42,
            emblem_id: 3,
            notice_subject: "Welcome".into(),
            notice_body: "Be kind".into(),
            positions: vec![GuildPositionInfo {
                index: 0,
                name: "Master".into(),
                can_invite: true,
                can_expel: true,
                can_storage: false,
                tax: 0,
            }],
            members: vec![
                GuildMemberInfo {
                    char_id: 42,
                    name: "Odin".into(),
                    job_id: 4008,
                    base_level: 99,
                    online: true,
                    map: "prontera".into(),
                    position_index: 0,
                    hp: 90,
                    max_hp: 100,
                    sp: 40,
                    max_sp: 50,
                    ap: 8,
                    max_ap: 10,
                },
                GuildMemberInfo {
                    char_id: 43,
                    name: "Thor".into(),
                    job_id: 999_999,
                    base_level: 80,
                    online: false,
                    map: "geffen".into(),
                    position_index: 0,
                    hp: 70,
                    max_hp: 100,
                    sp: 30,
                    max_sp: 50,
                    ap: 5,
                    max_ap: 10,
                },
            ],
        };

        let rows = super::project_rows(&info, None);

        assert_eq!(rows[0].name, "Odin");
        assert_eq!(rows[0].position, "Master");
        assert_eq!(rows[0].job, "Unknown Job");
        assert!(rows[0].online);
        assert_eq!(rows[0].map, "prontera");
        assert_eq!(rows[0].hp, (90, 100));
        assert_eq!(rows[0].ap, Some((8, 10)));
        assert!(!rows[1].online);
        assert_eq!(rows[1].map, "geffen");
        assert_eq!(rows[1].hp, (70, 100));
        assert_eq!(rows[1].ap, Some((5, 10)));
    }
}
