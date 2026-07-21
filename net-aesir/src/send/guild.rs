use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{
    GuildCreateRequested, GuildEmblemFetchRequested, GuildEmblemUploadRequested,
    GuildExpelRequested, GuildInviteRequested, GuildInviteResponded, GuildLeaveRequested,
    GuildMemberPositionRequested, GuildNoticeEditRequested, GuildPositionEditRequested,
};

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::{
    GuildCreateRequest, GuildEmblemRequest, GuildEmblemUploadRequest, GuildExpelRequest,
    GuildInviteRequest, GuildInviteResponse, GuildLeaveRequest, GuildMemberPositionRequest,
    GuildNoticeEditRequest, GuildPositionEditRequest,
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

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Last,
    config(run_if = not(client_connected))
)]
#[allow(clippy::too_many_arguments)]
pub fn clear_guild_commands_while_disconnected(
    mut creates: ResMut<Messages<GuildCreateRequested>>,
    mut invites: ResMut<Messages<GuildInviteRequested>>,
    mut invite_responses: ResMut<Messages<GuildInviteResponded>>,
    mut leaves: ResMut<Messages<GuildLeaveRequested>>,
    mut expulsions: ResMut<Messages<GuildExpelRequested>>,
    mut position_edits: ResMut<Messages<GuildPositionEditRequested>>,
    mut member_positions: ResMut<Messages<GuildMemberPositionRequested>>,
    mut notice_edits: ResMut<Messages<GuildNoticeEditRequested>>,
    mut emblem_uploads: ResMut<Messages<GuildEmblemUploadRequested>>,
    mut emblem_fetches: ResMut<Messages<GuildEmblemFetchRequested>>,
) {
    creates.clear();
    invites.clear();
    invite_responses.clear();
    leaves.clear();
    expulsions.clear();
    position_edits.clear();
    member_positions.clear();
    notice_edits.clear();
    emblem_uploads.clear();
    emblem_fetches.clear();
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
        });

        match body {
            Body::GuildPositionEditRequest(GuildPositionEditRequest {
                index,
                name,
                can_invite,
                can_expel,
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
