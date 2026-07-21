use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use net_contract::{
    dto::GuildErrorKind,
    events::{GuildIngress, GuildIngressPayload},
    state::ZoneSessionGeneration,
};

use crate::{
    dispatch::IncomingMessage,
    zone::mapping::guild::{guild_payload, guild_scope_id},
};

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(
        after = crate::zone::session::publish_zone_session,
        run_if = client_connected
    )
)]
pub fn zone_drain_guild(
    generation: Res<ZoneSessionGeneration>,
    mut observed_generation: Local<Option<ZoneSessionGeneration>>,
    mut incoming: MessageReader<IncomingMessage>,
    mut out: MessageWriter<GuildIngress>,
) {
    if observed_generation.is_some_and(|previous| previous != *generation) {
        incoming.clear();
        *observed_generation = Some(*generation);
        return;
    }
    *observed_generation = Some(*generation);
    for message in incoming.read() {
        if guild_scope_id(&message.body) == Some(0) {
            warn!("dropping guild packet with zero guild id");
            continue;
        }
        if matches!(
            &message.body,
            crate::envelope::Body::GuildMemberUpdate(update) if update.member.is_none()
        ) {
            warn!("dropping guild member update without member payload");
            continue;
        }
        let Some(payload) = guild_payload(message.body.clone()) else {
            continue;
        };
        if let GuildIngressPayload::ActionResult(result) = &payload
            && let GuildErrorKind::Unknown(value) = result.error
        {
            warn!("unknown guild error value {value}");
        }
        out.write(GuildIngress {
            generation: *generation,
            payload,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{channels::GAMEPLAY, dispatch::IncomingMessage, envelope::Body};
    use net_contract::{
        dto::{GuildActionResult, GuildErrorKind, GuildInviteInfo},
        events::{GuildIngress, GuildIngressPayload},
        state::ZoneSessionGeneration,
    };

    #[test]
    fn guild_packets_keep_receive_order_and_active_generation() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<GuildIngress>()
            .insert_resource(ZoneSessionGeneration(9))
            .add_systems(Update, zone_drain_guild);

        let bodies = [
            Body::GuildActionResult(crate::proto::aesir::net::GuildActionResult {
                action: "create".into(),
                success: true,
                error: 0,
            }),
            Body::GuildInviteNotify(crate::proto::aesir::net::GuildInviteNotify {
                guild_id: 7,
                guild_name: "Vikings".into(),
                inviter_name: "Odin".into(),
            }),
            Body::GuildInfo(crate::proto::aesir::net::GuildInfo {
                guild_id: 7,
                name: "Vikings".into(),
                master_char_id: 42,
                emblem_id: 3,
                notice_subject: String::new(),
                notice_body: String::new(),
                positions: vec![],
                members: vec![],
            }),
            Body::GuildMemberUpdate(crate::proto::aesir::net::GuildMemberUpdate {
                guild_id: 7,
                member: Some(crate::proto::aesir::net::GuildMember {
                    char_id: 43,
                    name: "Thor".into(),
                    job_id: 7,
                    base_level: 50,
                    online: true,
                    map: "prontera".into(),
                    position_index: 2,
                    hp: 8,
                    max_hp: 9,
                    sp: 10,
                    max_sp: 11,
                    ap: 12,
                    max_ap: 13,
                }),
            }),
            Body::GuildEmblemChanged(crate::proto::aesir::net::GuildEmblemChanged {
                guild_id: 7,
                emblem_id: 4,
            }),
            Body::GuildEmblemData(crate::proto::aesir::net::GuildEmblemData {
                guild_id: 7,
                emblem_id: 4,
                data: vec![0x42, 0x4d],
            }),
            Body::GuildDisbanded(crate::proto::aesir::net::GuildDisbanded {
                guild_id: 7,
                reason: "master left".into(),
            }),
        ];
        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for body in bodies {
            incoming.write(IncomingMessage {
                channel: GAMEPLAY,
                body,
            });
        }

        app.update();

        let received = app.world().resource::<Messages<GuildIngress>>();
        let events: Vec<_> = received.iter_current_update_messages().collect();
        assert_eq!(events.len(), 7);
        assert!(
            events
                .iter()
                .all(|event| event.generation == ZoneSessionGeneration(9))
        );
        assert_eq!(
            events[0].payload,
            GuildIngressPayload::ActionResult(GuildActionResult {
                action: "create".into(),
                success: true,
                error: GuildErrorKind::None,
            })
        );
        assert_eq!(
            events[1].payload,
            GuildIngressPayload::InviteNotified(GuildInviteInfo {
                guild_id: 7,
                guild_name: "Vikings".into(),
                inviter_name: "Odin".into(),
            })
        );
        assert!(matches!(events[2].payload, GuildIngressPayload::Info(_)));
        assert!(matches!(
            events[3].payload,
            GuildIngressPayload::MemberUpdated { .. }
        ));
        assert!(matches!(
            events[4].payload,
            GuildIngressPayload::EmblemChanged { .. }
        ));
        assert!(matches!(
            events[5].payload,
            GuildIngressPayload::EmblemData { .. }
        ));
        assert!(matches!(
            events[6].payload,
            GuildIngressPayload::Disbanded { .. }
        ));
    }

    #[test]
    fn member_update_without_member_is_dropped() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<GuildIngress>()
            .insert_resource(ZoneSessionGeneration(3))
            .add_systems(Update, zone_drain_guild);
        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: GAMEPLAY,
                body: Body::GuildMemberUpdate(crate::proto::aesir::net::GuildMemberUpdate {
                    guild_id: 7,
                    member: None,
                }),
            });

        app.update();

        assert_eq!(
            app.world()
                .resource::<Messages<GuildIngress>>()
                .iter_current_update_messages()
                .count(),
            0
        );
    }

    #[test]
    fn generation_change_discards_unread_old_packets_before_accepting_new_packets() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<GuildIngress>()
            .insert_resource(ZoneSessionGeneration(1))
            .add_systems(Update, zone_drain_guild);

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: GAMEPLAY,
                body: Body::GuildInviteNotify(crate::proto::aesir::net::GuildInviteNotify {
                    guild_id: 7,
                    guild_name: "Character A".into(),
                    inviter_name: "Odin".into(),
                }),
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<Messages<GuildIngress>>()
                .iter_current_update_messages()
                .count(),
            1
        );

        *app.world_mut().resource_mut::<ZoneSessionGeneration>() = ZoneSessionGeneration(2);
        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: GAMEPLAY,
                body: Body::GuildInviteNotify(crate::proto::aesir::net::GuildInviteNotify {
                    guild_id: 8,
                    guild_name: "Character A stale".into(),
                    inviter_name: "Odin".into(),
                }),
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<Messages<GuildIngress>>()
                .iter_current_update_messages()
                .count(),
            0
        );

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: GAMEPLAY,
                body: Body::GuildInviteNotify(crate::proto::aesir::net::GuildInviteNotify {
                    guild_id: 9,
                    guild_name: "Character B".into(),
                    inviter_name: "Freya".into(),
                }),
            });
        app.update();
        let events = app
            .world()
            .resource::<Messages<GuildIngress>>()
            .iter_current_update_messages()
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].generation, ZoneSessionGeneration(2));
        assert!(matches!(
            &events[0].payload,
            GuildIngressPayload::InviteNotified(invite) if invite.guild_id == 9
        ));
    }

    #[test]
    fn zero_guild_ids_emit_no_ingress() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<GuildIngress>()
            .insert_resource(ZoneSessionGeneration(3))
            .add_systems(Update, zone_drain_guild);
        let bodies = [
            Body::GuildInviteNotify(crate::proto::aesir::net::GuildInviteNotify {
                guild_id: 0,
                guild_name: "Invalid".into(),
                inviter_name: "Odin".into(),
            }),
            Body::GuildInfo(crate::proto::aesir::net::GuildInfo {
                guild_id: 0,
                name: "Invalid".into(),
                master_char_id: 42,
                emblem_id: 1,
                notice_subject: String::new(),
                notice_body: String::new(),
                positions: vec![],
                members: vec![],
            }),
            Body::GuildMemberUpdate(crate::proto::aesir::net::GuildMemberUpdate {
                guild_id: 0,
                member: Some(crate::proto::aesir::net::GuildMember {
                    char_id: 42,
                    name: "Odin".into(),
                    job_id: 1,
                    base_level: 1,
                    online: true,
                    map: "prontera".into(),
                    position_index: 0,
                    hp: 1,
                    max_hp: 1,
                    sp: 1,
                    max_sp: 1,
                    ap: 0,
                    max_ap: 0,
                }),
            }),
            Body::GuildEmblemChanged(crate::proto::aesir::net::GuildEmblemChanged {
                guild_id: 0,
                emblem_id: 1,
            }),
            Body::GuildEmblemData(crate::proto::aesir::net::GuildEmblemData {
                guild_id: 0,
                emblem_id: 1,
                data: vec![0x42, 0x4d],
            }),
            Body::GuildDisbanded(crate::proto::aesir::net::GuildDisbanded {
                guild_id: 0,
                reason: "invalid".into(),
            }),
        ];
        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for body in bodies {
            incoming.write(IncomingMessage {
                channel: GAMEPLAY,
                body,
            });
        }

        app.update();

        assert_eq!(
            app.world()
                .resource::<Messages<GuildIngress>>()
                .iter_current_update_messages()
                .count(),
            0
        );
    }
}
