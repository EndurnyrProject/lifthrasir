use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{
    GuildAllianceBreakRequested, GuildAllianceRequested, GuildAllianceResponded,
    GuildAntagonistRemoveRequested, GuildAntagonistRequested, GuildCreateRequested,
    GuildEmblemFetchRequested, GuildEmblemUploadRequested, GuildExpelRequested,
    GuildInviteRequested, GuildInviteResponded, GuildLeaveRequested, GuildMemberPositionRequested,
    GuildNoticeEditRequested, GuildPositionEditRequested, GuildSkillUpRequested,
};

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::{
    GuildAllianceBreakRequest, GuildAllianceRequest, GuildAllianceResponse,
    GuildAntagonistRemoveRequest, GuildAntagonistRequest, GuildCreateRequest, GuildEmblemRequest,
    GuildEmblemUploadRequest, GuildExpelRequest, GuildInviteRequest, GuildInviteResponse,
    GuildLeaveRequest, GuildMemberPositionRequest, GuildNoticeEditRequest,
    GuildPositionEditRequest, GuildSkillUpRequest,
};
use crate::zone::{QuicZoneState, ZonePhase};

fn guild_create_body(command: &GuildCreateRequested) -> Body {
    Body::GuildCreateRequest(GuildCreateRequest {
        name: command.name.clone(),
    })
}

fn guild_invite_body(command: &GuildInviteRequested) -> Body {
    Body::GuildInviteRequest(GuildInviteRequest {
        target_char_id: command.target_char_id,
        target_name: command.target_name.clone(),
    })
}

fn guild_invite_response_body(command: &GuildInviteResponded) -> Body {
    Body::GuildInviteResponse(GuildInviteResponse {
        guild_id: command.guild_id,
        accept: command.accept,
    })
}

fn guild_leave_body(_command: &GuildLeaveRequested) -> Body {
    Body::GuildLeaveRequest(GuildLeaveRequest {})
}

fn guild_expel_body(command: &GuildExpelRequested) -> Body {
    Body::GuildExpelRequest(GuildExpelRequest {
        target_char_id: command.target_char_id,
        reason: command.reason.clone(),
    })
}

fn guild_position_edit_body(command: &GuildPositionEditRequested) -> Body {
    Body::GuildPositionEditRequest(GuildPositionEditRequest {
        index: command.index,
        name: command.name.clone(),
        can_invite: command.can_invite,
        can_expel: command.can_expel,
        can_storage: command.can_storage,
        tax: command.tax,
    })
}

fn guild_skill_up_body(command: &GuildSkillUpRequested) -> Body {
    Body::GuildSkillUpRequest(GuildSkillUpRequest {
        skill_id: command.skill_id,
    })
}

fn guild_alliance_body(command: &GuildAllianceRequested) -> Body {
    Body::GuildAllianceRequest(GuildAllianceRequest {
        target_char_id: command.target_char_id,
        target_name: command.target_name.clone(),
    })
}

fn guild_alliance_response_body(command: &GuildAllianceResponded) -> Body {
    Body::GuildAllianceResponse(GuildAllianceResponse {
        guild_id: command.guild_id,
        accept: command.accept,
    })
}

fn guild_alliance_break_body(command: &GuildAllianceBreakRequested) -> Body {
    Body::GuildAllianceBreakRequest(GuildAllianceBreakRequest {
        guild_id: command.guild_id,
    })
}

fn guild_antagonist_body(command: &GuildAntagonistRequested) -> Body {
    Body::GuildAntagonistRequest(GuildAntagonistRequest {
        target_char_id: command.target_char_id,
        target_name: command.target_name.clone(),
    })
}

fn guild_antagonist_remove_body(command: &GuildAntagonistRemoveRequested) -> Body {
    Body::GuildAntagonistRemoveRequest(GuildAntagonistRemoveRequest {
        guild_id: command.guild_id,
    })
}

fn guild_member_position_body(command: &GuildMemberPositionRequested) -> Body {
    Body::GuildMemberPositionRequest(GuildMemberPositionRequest {
        target_char_id: command.target_char_id,
        index: command.index,
    })
}

fn guild_notice_edit_body(command: &GuildNoticeEditRequested) -> Body {
    Body::GuildNoticeEditRequest(GuildNoticeEditRequest {
        subject: command.subject.clone(),
        body: command.body.clone(),
    })
}

fn guild_emblem_upload_body(command: &GuildEmblemUploadRequested) -> Body {
    Body::GuildEmblemUploadRequest(GuildEmblemUploadRequest {
        data: command.data.clone(),
    })
}

fn guild_emblem_fetch_body(command: &GuildEmblemFetchRequested) -> Body {
    Body::GuildEmblemRequest(GuildEmblemRequest {
        guild_id: command.guild_id,
        emblem_id: command.emblem_id,
    })
}

/// The full set of outbound guild command queues, grouped so systems that touch
/// all of them take a single parameter instead of ten.
#[derive(SystemParam)]
pub struct GuildCommandQueues<'w> {
    creates: ResMut<'w, Messages<GuildCreateRequested>>,
    invites: ResMut<'w, Messages<GuildInviteRequested>>,
    invite_responses: ResMut<'w, Messages<GuildInviteResponded>>,
    leaves: ResMut<'w, Messages<GuildLeaveRequested>>,
    expulsions: ResMut<'w, Messages<GuildExpelRequested>>,
    position_edits: ResMut<'w, Messages<GuildPositionEditRequested>>,
    skill_ups: ResMut<'w, Messages<GuildSkillUpRequested>>,
    alliance_requests: ResMut<'w, Messages<GuildAllianceRequested>>,
    alliance_responses: ResMut<'w, Messages<GuildAllianceResponded>>,
    alliance_breaks: ResMut<'w, Messages<GuildAllianceBreakRequested>>,
    antagonist_requests: ResMut<'w, Messages<GuildAntagonistRequested>>,
    antagonist_removals: ResMut<'w, Messages<GuildAntagonistRemoveRequested>>,
    member_positions: ResMut<'w, Messages<GuildMemberPositionRequested>>,
    notice_edits: ResMut<'w, Messages<GuildNoticeEditRequested>>,
    emblem_uploads: ResMut<'w, Messages<GuildEmblemUploadRequested>>,
    emblem_fetches: ResMut<'w, Messages<GuildEmblemFetchRequested>>,
}

impl GuildCommandQueues<'_> {
    fn clear_all(&mut self) {
        self.creates.clear();
        self.invites.clear();
        self.invite_responses.clear();
        self.leaves.clear();
        self.expulsions.clear();
        self.position_edits.clear();
        self.skill_ups.clear();
        self.alliance_requests.clear();
        self.alliance_responses.clear();
        self.alliance_breaks.clear();
        self.antagonist_requests.clear();
        self.antagonist_removals.clear();
        self.member_positions.clear();
        self.notice_edits.clear();
        self.emblem_uploads.clear();
        self.emblem_fetches.clear();
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Last,
    config(run_if = not(client_connected))
)]
pub fn clear_guild_commands_while_disconnected(mut queues: GuildCommandQueues) {
    queues.clear_all();
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_create(
    mut commands: MessageReader<GuildCreateRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_create_body(command)) {
            error!("failed to send GuildCreateRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_invite(
    mut commands: MessageReader<GuildInviteRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_invite_body(command)) {
            error!("failed to send GuildInviteRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_invite_response(
    mut commands: MessageReader<GuildInviteResponded>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_invite_response_body(command)) {
            error!("failed to send GuildInviteResponse: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_leave(
    mut commands: MessageReader<GuildLeaveRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_leave_body(command)) {
            error!("failed to send GuildLeaveRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_expel(
    mut commands: MessageReader<GuildExpelRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_expel_body(command)) {
            error!("failed to send GuildExpelRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_position_edit(
    mut commands: MessageReader<GuildPositionEditRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_position_edit_body(command)) {
            error!("failed to send GuildPositionEditRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_skill_up(
    mut commands: MessageReader<GuildSkillUpRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_skill_up_body(command)) {
            error!("failed to send GuildSkillUpRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_alliance(
    mut commands: MessageReader<GuildAllianceRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_alliance_body(command)) {
            error!("failed to send GuildAllianceRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_alliance_response(
    mut commands: MessageReader<GuildAllianceResponded>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_alliance_response_body(command))
        {
            error!("failed to send GuildAllianceResponse: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_alliance_break(
    mut commands: MessageReader<GuildAllianceBreakRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_alliance_break_body(command)) {
            error!("failed to send GuildAllianceBreakRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_antagonist(
    mut commands: MessageReader<GuildAntagonistRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_antagonist_body(command)) {
            error!("failed to send GuildAntagonistRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_antagonist_remove(
    mut commands: MessageReader<GuildAntagonistRemoveRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_antagonist_remove_body(command))
        {
            error!("failed to send GuildAntagonistRemoveRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_member_position(
    mut commands: MessageReader<GuildMemberPositionRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_member_position_body(command)) {
            error!("failed to send GuildMemberPositionRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_notice_edit(
    mut commands: MessageReader<GuildNoticeEditRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_notice_edit_body(command)) {
            error!("failed to send GuildNoticeEditRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_emblem_upload(
    mut commands: MessageReader<GuildEmblemUploadRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_emblem_upload_body(command)) {
            error!("failed to send GuildEmblemUploadRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_guild_emblem_fetch(
    mut commands: MessageReader<GuildEmblemFetchRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, guild_emblem_fetch_body(command)) {
            error!("failed to send GuildEmblemRequest: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_guild_messages() -> App {
        let mut app = App::new();
        app.init_resource::<QuinnetClient>();
        app.init_resource::<QuicZoneState>();
        app.add_message::<GuildCreateRequested>();
        app.add_message::<GuildInviteRequested>();
        app.add_message::<GuildInviteResponded>();
        app.add_message::<GuildLeaveRequested>();
        app.add_message::<GuildExpelRequested>();
        app.add_message::<GuildPositionEditRequested>();
        app.add_message::<GuildSkillUpRequested>();
        app.add_message::<GuildAllianceRequested>();
        app.add_message::<GuildAllianceResponded>();
        app.add_message::<GuildAllianceBreakRequested>();
        app.add_message::<GuildAntagonistRequested>();
        app.add_message::<GuildAntagonistRemoveRequested>();
        app.add_message::<GuildMemberPositionRequested>();
        app.add_message::<GuildNoticeEditRequested>();
        app.add_message::<GuildEmblemUploadRequested>();
        app.add_message::<GuildEmblemFetchRequested>();
        app
    }

    fn app_with_guild_senders() -> App {
        let mut app = app_with_guild_messages();
        app.add_systems(
            Update,
            (
                send_guild_create,
                send_guild_invite,
                send_guild_invite_response,
                send_guild_leave,
                send_guild_expel,
                send_guild_position_edit,
                send_guild_member_position,
                send_guild_notice_edit,
                send_guild_emblem_upload,
                send_guild_emblem_fetch,
            ),
        );
        app.add_systems(
            Last,
            clear_guild_commands_while_disconnected.run_if(not(client_connected)),
        );
        app
    }

    fn write_all_commands(app: &mut App) {
        app.world_mut().write_message(GuildCreateRequested {
            name: "Heroes".to_string(),
        });
        app.world_mut().write_message(GuildInviteRequested {
            target_char_id: 1,
            target_name: String::new(),
        });
        app.world_mut().write_message(GuildInviteResponded {
            guild_id: 2,
            accept: true,
        });
        app.world_mut().write_message(GuildLeaveRequested);
        app.world_mut().write_message(GuildExpelRequested {
            target_char_id: 3,
            reason: "reason".to_string(),
        });
        app.world_mut().write_message(GuildPositionEditRequested {
            index: 4,
            name: "Officer".to_string(),
            can_invite: true,
            can_expel: true,
            tax: None,
            can_storage: None,
        });
        app.world_mut().write_message(GuildMemberPositionRequested {
            target_char_id: 5,
            index: 6,
        });
        app.world_mut().write_message(GuildNoticeEditRequested {
            subject: "subject".to_string(),
            body: "body".to_string(),
        });
        app.world_mut()
            .write_message(GuildEmblemUploadRequested { data: vec![1, 2] });
        app.world_mut().write_message(GuildEmblemFetchRequested {
            guild_id: 7,
            emblem_id: 8,
        });
    }

    #[test]
    fn skill_up_body_preserves_skill_id() {
        let body =
            guild_skill_up_body(&net_contract::commands::GuildSkillUpRequested { skill_id: 10001 });

        match body {
            Body::GuildSkillUpRequest(crate::proto::aesir::net::GuildSkillUpRequest {
                skill_id,
            }) => assert_eq!(skill_id, 10001),
            other => panic!("expected Body::GuildSkillUpRequest, got {other:?}"),
        }
    }

    #[test]
    fn alliance_body_preserves_character_target() {
        let body = guild_alliance_body(&GuildAllianceRequested {
            target_char_id: 42,
            target_name: "Ally".to_string(),
        });

        match body {
            Body::GuildAllianceRequest(GuildAllianceRequest {
                target_char_id,
                target_name,
            }) => {
                assert_eq!(target_char_id, 42);
                assert_eq!(target_name, "Ally");
            }
            other => panic!("expected Body::GuildAllianceRequest, got {other:?}"),
        }
    }

    #[test]
    fn alliance_response_body_preserves_decline() {
        let body = guild_alliance_response_body(&GuildAllianceResponded {
            guild_id: 43,
            accept: false,
        });

        match body {
            Body::GuildAllianceResponse(GuildAllianceResponse { guild_id, accept }) => {
                assert_eq!(guild_id, 43);
                assert!(!accept);
            }
            other => panic!("expected Body::GuildAllianceResponse, got {other:?}"),
        }
    }

    #[test]
    fn alliance_break_body_preserves_guild_id() {
        assert!(matches!(
            guild_alliance_break_body(&GuildAllianceBreakRequested { guild_id: 44 }),
            Body::GuildAllianceBreakRequest(GuildAllianceBreakRequest { guild_id: 44 })
        ));
    }

    #[test]
    fn antagonist_body_preserves_character_target() {
        let body = guild_antagonist_body(&GuildAntagonistRequested {
            target_char_id: 45,
            target_name: "Rival".to_string(),
        });

        match body {
            Body::GuildAntagonistRequest(GuildAntagonistRequest {
                target_char_id,
                target_name,
            }) => {
                assert_eq!(target_char_id, 45);
                assert_eq!(target_name, "Rival");
            }
            other => panic!("expected Body::GuildAntagonistRequest, got {other:?}"),
        }
    }

    #[test]
    fn antagonist_remove_body_preserves_guild_id() {
        assert!(matches!(
            guild_antagonist_remove_body(&GuildAntagonistRemoveRequested { guild_id: 46 }),
            Body::GuildAntagonistRemoveRequest(GuildAntagonistRemoveRequest { guild_id: 46 })
        ));
    }

    #[test]
    fn create_body_carries_name() {
        let body = guild_create_body(&GuildCreateRequested {
            name: "Heroes".to_string(),
        });

        match body {
            Body::GuildCreateRequest(GuildCreateRequest { name }) => {
                assert_eq!(name, "Heroes")
            }
            other => panic!("expected Body::GuildCreateRequest, got {other:?}"),
        }
    }

    #[test]
    fn invite_body_maps_target_fields() {
        let body = guild_invite_body(&GuildInviteRequested {
            target_char_id: 42,
            target_name: "Ally".to_string(),
        });

        match body {
            Body::GuildInviteRequest(GuildInviteRequest {
                target_char_id,
                target_name,
            }) => {
                assert_eq!(target_char_id, 42);
                assert_eq!(target_name, "Ally");
            }
            other => panic!("expected Body::GuildInviteRequest, got {other:?}"),
        }
    }

    #[test]
    fn invite_response_body_maps_guild_and_choice() {
        let body = guild_invite_response_body(&GuildInviteResponded {
            guild_id: 7,
            accept: true,
        });

        match body {
            Body::GuildInviteResponse(GuildInviteResponse { guild_id, accept }) => {
                assert_eq!(guild_id, 7);
                assert!(accept);
            }
            other => panic!("expected Body::GuildInviteResponse, got {other:?}"),
        }
    }

    #[test]
    fn leave_body_is_empty() {
        assert!(matches!(
            guild_leave_body(&GuildLeaveRequested),
            Body::GuildLeaveRequest(GuildLeaveRequest {})
        ));
    }

    #[test]
    fn expel_body_preserves_target_and_reason() {
        let body = guild_expel_body(&GuildExpelRequested {
            target_char_id: 88,
            reason: "Repeated griefing".to_string(),
        });

        match body {
            Body::GuildExpelRequest(GuildExpelRequest {
                target_char_id,
                reason,
            }) => {
                assert_eq!(target_char_id, 88);
                assert_eq!(reason, "Repeated griefing");
            }
            other => panic!("expected Body::GuildExpelRequest, got {other:?}"),
        }
    }

    #[test]
    fn position_edit_body_preserves_slot_permissions_and_name() {
        let body = guild_position_edit_body(&GuildPositionEditRequested {
            index: 3,
            name: "Officer".to_string(),
            can_invite: true,
            can_expel: false,
            tax: None,
            can_storage: None,
        });

        match body {
            Body::GuildPositionEditRequest(GuildPositionEditRequest {
                index,
                name,
                can_invite,
                can_expel,
                can_storage: _,
                tax: _,
            }) => {
                assert_eq!(index, 3);
                assert_eq!(name, "Officer");
                assert!(can_invite);
                assert!(!can_expel);
            }
            other => panic!("expected Body::GuildPositionEditRequest, got {other:?}"),
        }
    }

    #[test]
    fn position_edit_body_preserves_optional_storage_and_tax_intent() {
        let cases = [(None, None), (Some(false), Some(0)), (Some(true), Some(25))];

        for (can_storage, tax) in cases {
            let body = guild_position_edit_body(&GuildPositionEditRequested {
                index: 3,
                name: "Officer".to_string(),
                can_invite: true,
                can_expel: false,
                can_storage,
                tax,
            });
            let Body::GuildPositionEditRequest(request) = body else {
                panic!("expected GuildPositionEditRequest");
            };
            assert_eq!(request.can_storage, can_storage);
            assert_eq!(request.tax, tax);
        }
    }

    #[test]
    fn member_position_body_preserves_target_and_slot() {
        let body = guild_member_position_body(&GuildMemberPositionRequested {
            target_char_id: 99,
            index: 4,
        });

        match body {
            Body::GuildMemberPositionRequest(GuildMemberPositionRequest {
                target_char_id,
                index,
            }) => {
                assert_eq!(target_char_id, 99);
                assert_eq!(index, 4);
            }
            other => panic!("expected Body::GuildMemberPositionRequest, got {other:?}"),
        }
    }

    #[test]
    fn notice_body_preserves_subject_and_body() {
        let body = guild_notice_edit_body(&GuildNoticeEditRequested {
            subject: "Raid".to_string(),
            body: "Saturday at 20:00".to_string(),
        });

        match body {
            Body::GuildNoticeEditRequest(GuildNoticeEditRequest { subject, body }) => {
                assert_eq!(subject, "Raid");
                assert_eq!(body, "Saturday at 20:00");
            }
            other => panic!("expected Body::GuildNoticeEditRequest, got {other:?}"),
        }
    }

    #[test]
    fn emblem_upload_body_preserves_original_bytes() {
        let bytes = vec![0x42, 0x4d, 0x00, 0xff, 0x7f];
        let body = guild_emblem_upload_body(&GuildEmblemUploadRequested {
            data: bytes.clone(),
        });

        match body {
            Body::GuildEmblemUploadRequest(GuildEmblemUploadRequest { data }) => {
                assert_eq!(data, bytes);
            }
            other => panic!("expected Body::GuildEmblemUploadRequest, got {other:?}"),
        }
    }

    #[test]
    fn emblem_fetch_body_preserves_guild_and_version() {
        let body = guild_emblem_fetch_body(&GuildEmblemFetchRequested {
            guild_id: 123,
            emblem_id: 456,
        });

        match body {
            Body::GuildEmblemRequest(GuildEmblemRequest {
                guild_id,
                emblem_id,
            }) => {
                assert_eq!(guild_id, 123);
                assert_eq!(emblem_id, 456);
            }
            other => panic!("expected Body::GuildEmblemRequest, got {other:?}"),
        }
    }

    fn assert_all_commands_empty(app: &App) {
        assert!(
            app.world()
                .resource::<Messages<GuildCreateRequested>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildInviteRequested>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildInviteResponded>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildLeaveRequested>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildExpelRequested>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildPositionEditRequested>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildMemberPositionRequested>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildNoticeEditRequested>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildEmblemUploadRequested>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildEmblemFetchRequested>>()
                .is_empty()
        );
    }

    fn write_skill_and_relation_commands(app: &mut App) {
        app.world_mut()
            .write_message(GuildSkillUpRequested { skill_id: 1 });
        app.world_mut().write_message(GuildAllianceRequested {
            target_char_id: 2,
            target_name: "Ally".to_string(),
        });
        app.world_mut().write_message(GuildAllianceResponded {
            guild_id: 3,
            accept: false,
        });
        app.world_mut()
            .write_message(GuildAllianceBreakRequested { guild_id: 4 });
        app.world_mut().write_message(GuildAntagonistRequested {
            target_char_id: 5,
            target_name: "Rival".to_string(),
        });
        app.world_mut()
            .write_message(GuildAntagonistRemoveRequested { guild_id: 6 });
    }

    fn assert_skill_and_relation_commands_empty(app: &App) {
        assert!(
            app.world()
                .resource::<Messages<GuildSkillUpRequested>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildAllianceRequested>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildAllianceResponded>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildAllianceBreakRequested>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildAntagonistRequested>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildAntagonistRemoveRequested>>()
                .is_empty()
        );
    }

    #[test]
    fn skill_and_relation_commands_are_consumed_outside_playing() {
        let mut app = app_with_guild_messages();
        app.add_systems(
            Update,
            (
                send_guild_skill_up,
                send_guild_alliance,
                send_guild_alliance_response,
                send_guild_alliance_break,
                send_guild_antagonist,
                send_guild_antagonist_remove,
            ),
        );
        app.update();
        write_skill_and_relation_commands(&mut app);

        app.update();
        app.world_mut().resource_mut::<QuicZoneState>().phase = ZonePhase::Playing;
        app.update();

        let frame = app
            .world_mut()
            .resource_mut::<QuicZoneState>()
            .conn
            .next_frame(Body::GuildSkillUpRequest(GuildSkillUpRequest {
                skill_id: 0,
            }));
        assert_eq!(crate::envelope::decode(&frame).unwrap().seq, 0);
    }

    #[test]
    fn skill_and_relation_commands_are_cleared_while_disconnected() {
        let mut app = app_with_guild_messages();
        app.add_systems(
            Last,
            clear_guild_commands_while_disconnected.run_if(not(client_connected)),
        );
        app.world_mut().resource_mut::<QuicZoneState>().phase = ZonePhase::Playing;
        write_skill_and_relation_commands(&mut app);

        app.update();

        assert_skill_and_relation_commands_empty(&app);
    }

    #[test]
    fn disconnected_commands_are_drained_before_playing_reconnect() {
        let mut app = app_with_guild_senders();
        write_all_commands(&mut app);

        app.update();
        app.world_mut().resource_mut::<QuicZoneState>().phase = ZonePhase::Playing;
        app.update();

        assert_all_commands_empty(&app);
    }

    #[test]
    fn commands_queued_while_disconnected_are_consumed() {
        let mut app = app_with_guild_messages();
        app.add_systems(
            Last,
            clear_guild_commands_while_disconnected.run_if(not(client_connected)),
        );
        app.world_mut().resource_mut::<QuicZoneState>().phase = ZonePhase::Playing;
        write_all_commands(&mut app);

        app.update();

        assert_all_commands_empty(&app);
    }
}
