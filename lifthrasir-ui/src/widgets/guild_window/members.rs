use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use game_engine::infrastructure::job::JobSpriteRegistry;
use net_contract::commands::GuildInviteRequested;
use net_contract::dto::GuildInfo;
use net_contract::state::ZoneSessionGeneration;

use super::{
    dialogs::PendingGuildConfirmation, GuildInviteNameField, GuildMutationContext, GuildUi,
    PendingGuildMutation,
};

#[derive(Component, Clone, Default)]
pub(crate) struct GuildExpelControl(pub u32);

#[derive(Component, Clone, Default)]
pub(crate) struct GuildExpelButton(pub u32);

#[derive(Component, Clone, Default)]
pub(crate) struct GuildExpelReasonField(pub u32);

pub(crate) fn request_invite(
    ui: &mut GuildUi,
    generation: ZoneSessionGeneration,
    target_char_id: u32,
    raw_name: &str,
) -> Option<GuildInviteRequested> {
    if ui.pending.is_some() {
        ui.feedback = Some("A guild action is already pending.".into());
        ui.feedback_is_error = true;
        return None;
    }
    let target_name = raw_name.trim();
    if (target_char_id == 0) == target_name.is_empty() {
        ui.feedback = Some("Enter a character name.".into());
        ui.feedback_is_error = true;
        return None;
    }
    ui.pending = Some(PendingGuildMutation {
        action: "invite",
        generation,
    });
    ui.feedback = Some("Sending guild invitation…".into());
    ui.feedback_is_error = false;
    Some(GuildInviteRequested {
        target_char_id,
        target_name: target_name.into(),
    })
}

pub(crate) fn on_invite_by_name(
    _: On<Activate>,
    field: Query<&EditableText, With<GuildInviteNameField>>,
    generation: Res<ZoneSessionGeneration>,
    mut ui: ResMut<GuildUi>,
    mut writer: MessageWriter<GuildInviteRequested>,
) {
    let Ok(field) = field.single() else {
        return;
    };
    if let Some(command) = request_invite(&mut ui, *generation, 0, &field.value().to_string()) {
        writer.write(command);
    }
}

pub(crate) fn on_expel(
    activate: On<Activate>,
    buttons: Query<&GuildExpelButton>,
    fields: Query<(&GuildExpelReasonField, &EditableText)>,
    mut context: GuildMutationContext,
    mut pending: ResMut<PendingGuildConfirmation>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    if context.ui.pending.is_some() || pending.is_pending() {
        return;
    }
    if button.0 == context.session.char_id
        || !context.guild.can_expel(context.session.char_id)
        || context.guild.is_master(button.0)
        || context.guild.member(button.0).is_none()
    {
        return;
    }
    let Some((_, field)) = fields.iter().find(|(field, _)| field.0 == button.0) else {
        return;
    };
    let reason = field.value().to_string();
    let reason = reason.trim();
    if reason.is_empty() {
        context.ui.feedback = Some("Enter a reason before expelling a member.".into());
        context.ui.feedback_is_error = true;
        return;
    }
    let Some(target_name) = context
        .guild
        .member(button.0)
        .map(|member| member.name.clone())
    else {
        return;
    };
    pending.expel(*context.generation, button.0, &target_name, reason);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MemberRow {
    pub char_id: u32,
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
            char_id: member.char_id,
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
    use super::{request_invite, GuildUi};
    use net_contract::dto::{GuildInfo, GuildMemberInfo, GuildPositionInfo};
    use net_contract::state::ZoneSessionGeneration;

    #[test]
    fn by_name_and_character_id_use_the_same_neutral_command() {
        let generation = ZoneSessionGeneration(3);
        let by_name = request_invite(&mut GuildUi::default(), generation, 0, "  Thor ").unwrap();
        let by_id = request_invite(&mut GuildUi::default(), generation, 42, "").unwrap();

        assert_eq!(by_name.target_char_id, 0);
        assert_eq!(by_name.target_name, "Thor");
        assert_eq!(by_id.target_char_id, 42);
        assert_eq!(by_id.target_name, "");
    }

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
