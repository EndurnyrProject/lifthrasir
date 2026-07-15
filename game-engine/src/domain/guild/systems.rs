use super::{plugin::GuildSessionGate, resource::GuildState};
use bevy::prelude::*;
use net_contract::{
    events::{GuildIngress, GuildIngressPayload, ZoneDisconnected},
    state::{ZoneSession, ZoneSessionGeneration},
};

pub(super) fn reset_guild_session(
    generation: Res<ZoneSessionGeneration>,
    zone_session: Option<Res<ZoneSession>>,
    mut disconnected: MessageReader<ZoneDisconnected>,
    mut gate: ResMut<GuildSessionGate>,
    mut state: ResMut<GuildState>,
) {
    let char_id = zone_session.as_deref().map_or(0, |session| session.char_id);
    let generation_changed = gate.generation != *generation;
    let character_changed = gate.char_id != char_id;
    let disconnected = disconnected.read().count() != 0;
    if generation_changed || character_changed {
        gate.generation = *generation;
        gate.char_id = char_id;
        gate.blocked = character_changed && !generation_changed;
        *state = GuildState::default();
    }

    if disconnected {
        gate.blocked = !generation_changed;
        *state = GuildState::default();
    }
}

pub(super) fn block_guild_on_character_select(
    mut gate: ResMut<GuildSessionGate>,
    mut state: ResMut<GuildState>,
) {
    gate.blocked = true;
    *state = GuildState::default();
}

pub(super) fn apply_guild_ingress(
    generation: Res<ZoneSessionGeneration>,
    gate: Res<GuildSessionGate>,
    mut ingress: MessageReader<GuildIngress>,
    mut state: ResMut<GuildState>,
) {
    for event in ingress.read() {
        if gate.blocked || event.generation != *generation {
            continue;
        }

        match &event.payload {
            GuildIngressPayload::Info(info) => {
                if info.guild_id == 0 {
                    warn!("dropping guild snapshot with zero guild id");
                    continue;
                }
                if info.members.iter().any(|member| member.char_id == 0) {
                    warn!("dropping guild snapshot with zero member character id");
                    continue;
                }
                state.replace(info.clone());
            }
            GuildIngressPayload::MemberUpdated { guild_id, member }
                if state.info().is_some_and(|info| info.guild_id == *guild_id)
                    && member.char_id != 0 =>
            {
                state.update_member(member.clone());
            }
            GuildIngressPayload::EmblemChanged {
                guild_id,
                emblem_id,
            } if state.info().is_some_and(|info| info.guild_id == *guild_id) => {
                state.update_emblem(*emblem_id);
            }
            GuildIngressPayload::ActionResult(result)
                if result.success && result.action == "leave" =>
            {
                state.clear();
            }
            GuildIngressPayload::ActionResult(result)
                if result.success && result.action == "expel" =>
            {
                // NOTE: Clear an expelled local player's GuildState here once Aesir provides
                // an authoritative membership-ended event.
            }
            GuildIngressPayload::Disbanded { guild_id, .. }
                if state.info().is_some_and(|info| info.guild_id == *guild_id) =>
            {
                state.clear();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::state::GameState;
    use crate::domain::guild::{GuildPlugin, GuildState};
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;
    use net_contract::{
        dto::{GuildActionResult, GuildErrorKind, GuildInfo, GuildMemberInfo, GuildPositionInfo},
        events::{GuildIngress, GuildIngressPayload, ZoneDisconnected},
        state::{ZoneSession, ZoneSessionGeneration},
    };

    fn member(char_id: u32, position_index: u32) -> GuildMemberInfo {
        GuildMemberInfo {
            char_id,
            name: format!("Member {char_id}"),
            job_id: 1,
            base_level: 50,
            online: true,
            map: "prontera".into(),
            position_index,
            hp: 100,
            max_hp: 100,
            sp: 50,
            max_sp: 50,
            ap: 0,
            max_ap: 0,
        }
    }

    fn info(guild_id: u32, member: GuildMemberInfo) -> GuildInfo {
        GuildInfo {
            guild_id,
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
            members: vec![member],
        }
    }

    fn app(generation: u64) -> App {
        let mut app = App::new();
        app.add_message::<GuildIngress>()
            .add_message::<ZoneDisconnected>()
            .insert_resource(ZoneSessionGeneration(generation))
            .add_plugins(GuildPlugin);
        app
    }

    fn ingress(app: &mut App, generation: u64, payload: GuildIngressPayload) {
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(generation),
            payload,
        });
    }

    #[test]
    fn full_snapshot_replaces_authoritative_state() {
        let mut app = app(1);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );

        app.update();

        let state = app.world().resource::<GuildState>();
        assert_eq!(state.info().map(|info| info.guild_id), Some(7));
        assert_eq!(
            state.member(42).map(|member| member.name.as_str()),
            Some("Member 42")
        );
    }

    #[test]
    fn malformed_snapshot_does_not_replace_valid_state() {
        let mut app = app(1);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();

        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(0, member(42, 0))),
        );
        app.update();

        assert_eq!(
            app.world()
                .resource::<GuildState>()
                .info()
                .map(|info| info.guild_id),
            Some(7)
        );
    }

    #[test]
    fn snapshot_with_zero_member_identity_does_not_replace_valid_state() {
        let mut app = app(1);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();

        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(8, member(0, 0))),
        );
        app.update();

        let state = app.world().resource::<GuildState>();
        assert_eq!(state.info().map(|info| info.guild_id), Some(7));
        assert!(state.member(42).is_some());
        assert!(state.member(0).is_none());
    }

    #[test]
    fn matching_member_update_replaces_the_complete_roster_entry() {
        let mut app = app(1);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();

        let mut updated = member(42, 0);
        updated.name = "Odin".into();
        updated.hp = 75;
        updated.map = "geffen".into();
        ingress(
            &mut app,
            1,
            GuildIngressPayload::MemberUpdated {
                guild_id: 7,
                member: updated.clone(),
            },
        );
        app.update();

        assert_eq!(
            app.world().resource::<GuildState>().member(42),
            Some(&updated)
        );
    }

    #[test]
    fn wrong_guild_unknown_and_malformed_member_updates_are_ignored() {
        let mut app = app(1);
        let original = member(42, 0);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, original.clone())),
        );
        app.update();

        for (guild_id, member) in [(8, member(42, 0)), (7, member(99, 0)), (7, member(0, 0))] {
            ingress(
                &mut app,
                1,
                GuildIngressPayload::MemberUpdated { guild_id, member },
            );
        }
        app.update();

        let state = app.world().resource::<GuildState>();
        assert_eq!(
            state.info().map(|info| info.members.as_slice()),
            Some([original].as_slice())
        );
    }

    #[test]
    fn only_matching_emblem_change_updates_the_snapshot_version() {
        let mut app = app(1);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();

        ingress(
            &mut app,
            1,
            GuildIngressPayload::EmblemChanged {
                guild_id: 8,
                emblem_id: 10,
            },
        );
        ingress(
            &mut app,
            1,
            GuildIngressPayload::EmblemChanged {
                guild_id: 7,
                emblem_id: 11,
            },
        );
        app.update();

        assert_eq!(
            app.world()
                .resource::<GuildState>()
                .info()
                .map(|info| info.emblem_id),
            Some(11)
        );
    }

    #[test]
    fn only_successful_local_leave_clears_state() {
        let mut app = app(1);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();

        ingress(
            &mut app,
            1,
            GuildIngressPayload::ActionResult(GuildActionResult {
                action: "leave".into(),
                success: false,
                error: GuildErrorKind::NoPermission,
            }),
        );
        app.update();
        assert!(app.world().resource::<GuildState>().in_guild());

        ingress(
            &mut app,
            1,
            GuildIngressPayload::ActionResult(GuildActionResult {
                action: "leave".into(),
                success: true,
                error: GuildErrorKind::None,
            }),
        );
        app.update();
        assert!(!app.world().resource::<GuildState>().in_guild());
    }

    #[test]
    fn matching_disband_clears_state_idempotently() {
        let mut app = app(1);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();

        ingress(
            &mut app,
            1,
            GuildIngressPayload::Disbanded {
                guild_id: 8,
                reason: "other guild".into(),
            },
        );
        app.update();
        assert!(app.world().resource::<GuildState>().in_guild());

        for _ in 0..2 {
            ingress(
                &mut app,
                1,
                GuildIngressPayload::Disbanded {
                    guild_id: 7,
                    reason: "master left".into(),
                },
            );
        }
        app.update();

        assert!(!app.world().resource::<GuildState>().in_guild());
    }

    #[test]
    fn successful_expel_never_clears_the_expellers_state() {
        let mut app = app(1);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();

        ingress(
            &mut app,
            1,
            GuildIngressPayload::ActionResult(GuildActionResult {
                action: "expel".into(),
                success: true,
                error: GuildErrorKind::None,
            }),
        );
        app.update();

        assert_eq!(
            app.world()
                .resource::<GuildState>()
                .info()
                .map(|info| info.guild_id),
            Some(7)
        );
    }

    #[test]
    fn same_frame_snapshot_and_delta_follow_ingress_order() {
        let mut updated = member(42, 0);
        updated.hp = 25;

        let mut snapshot_then_delta = app(1);
        ingress(
            &mut snapshot_then_delta,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        ingress(
            &mut snapshot_then_delta,
            1,
            GuildIngressPayload::MemberUpdated {
                guild_id: 7,
                member: updated.clone(),
            },
        );
        snapshot_then_delta.update();
        assert_eq!(
            snapshot_then_delta
                .world()
                .resource::<GuildState>()
                .member(42)
                .map(|member| member.hp),
            Some(25)
        );

        let mut delta_then_snapshot = app(1);
        ingress(
            &mut delta_then_snapshot,
            1,
            GuildIngressPayload::MemberUpdated {
                guild_id: 7,
                member: updated,
            },
        );
        ingress(
            &mut delta_then_snapshot,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        delta_then_snapshot.update();
        assert_eq!(
            delta_then_snapshot
                .world()
                .resource::<GuildState>()
                .member(42)
                .map(|member| member.hp),
            Some(100)
        );
    }

    #[test]
    fn same_frame_snapshot_and_disband_follow_ingress_order() {
        let mut snapshot_then_disband = app(1);
        ingress(
            &mut snapshot_then_disband,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        ingress(
            &mut snapshot_then_disband,
            1,
            GuildIngressPayload::Disbanded {
                guild_id: 7,
                reason: "master left".into(),
            },
        );
        snapshot_then_disband.update();
        assert!(!snapshot_then_disband
            .world()
            .resource::<GuildState>()
            .in_guild());

        let mut disband_then_snapshot = app(1);
        ingress(
            &mut disband_then_snapshot,
            1,
            GuildIngressPayload::Disbanded {
                guild_id: 7,
                reason: "master left".into(),
            },
        );
        ingress(
            &mut disband_then_snapshot,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        disband_then_snapshot.update();
        assert_eq!(
            disband_then_snapshot
                .world()
                .resource::<GuildState>()
                .info()
                .map(|info| info.guild_id),
            Some(7)
        );
    }

    #[test]
    fn stale_generation_ingress_is_rejected() {
        let mut app = app(2);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );

        app.update();

        assert!(!app.world().resource::<GuildState>().in_guild());
    }

    #[test]
    fn session_generation_change_clears_without_new_ingress() {
        let mut app = app(1);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();

        *app.world_mut().resource_mut::<ZoneSessionGeneration>() = ZoneSessionGeneration(2);
        app.update();

        assert!(!app.world().resource::<GuildState>().in_guild());
    }

    #[test]
    fn character_a_ingress_cannot_mutate_character_b_state() {
        let mut app = app(1);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();

        *app.world_mut().resource_mut::<ZoneSessionGeneration>() = ZoneSessionGeneration(2);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(8, member(42, 0))),
        );
        ingress(
            &mut app,
            2,
            GuildIngressPayload::Info(info(9, member(43, 0))),
        );
        app.update();

        let state = app.world().resource::<GuildState>();
        assert_eq!(state.info().map(|info| info.guild_id), Some(9));
        assert!(state.member(43).is_some());
        assert!(state.member(42).is_none());
    }

    #[test]
    fn same_generation_character_change_blocks_ambiguous_ingress_until_a_fresh_epoch() {
        let mut app = app(1);
        app.world_mut().insert_resource(ZoneSession {
            char_id: 42,
            ..default()
        });
        app.update();
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();
        assert!(app.world().resource::<GuildState>().in_guild());

        app.world_mut().resource_mut::<ZoneSession>().char_id = 43;
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(8, member(42, 0))),
        );
        app.update();
        assert!(!app.world().resource::<GuildState>().in_guild());

        *app.world_mut().resource_mut::<ZoneSessionGeneration>() = ZoneSessionGeneration(2);
        ingress(
            &mut app,
            2,
            GuildIngressPayload::Info(info(9, member(43, 0))),
        );
        app.update();
        assert_eq!(
            app.world()
                .resource::<GuildState>()
                .info()
                .map(|info| info.guild_id),
            Some(9)
        );
    }

    #[test]
    fn disconnect_clears_and_blocks_same_generation_ingress() {
        let mut app = app(1);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();

        app.world_mut().write_message(ZoneDisconnected {
            reason: "closed".into(),
        });
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(8, member(42, 0))),
        );
        app.update();

        assert!(!app.world().resource::<GuildState>().in_guild());
    }

    #[test]
    fn fresh_epoch_after_disconnect_accepts_only_current_guild_info() {
        let mut app = app(1);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();

        app.world_mut().write_message(ZoneDisconnected {
            reason: "closed".into(),
        });
        app.update();
        *app.world_mut().resource_mut::<ZoneSessionGeneration>() = ZoneSessionGeneration(2);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(8, member(42, 0))),
        );
        ingress(
            &mut app,
            2,
            GuildIngressPayload::Info(info(9, member(42, 0))),
        );
        app.update();

        assert_eq!(
            app.world()
                .resource::<GuildState>()
                .info()
                .map(|info| info.guild_id),
            Some(9)
        );
    }

    #[test]
    fn connection_replacement_unblocks_when_disconnect_and_fresh_epoch_coincide() {
        let mut app = app(1);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();

        app.world_mut().write_message(ZoneDisconnected {
            reason: "replaced".into(),
        });
        *app.world_mut().resource_mut::<ZoneSessionGeneration>() = ZoneSessionGeneration(2);
        ingress(
            &mut app,
            2,
            GuildIngressPayload::Info(info(9, member(43, 0))),
        );
        app.update();

        assert_eq!(
            app.world()
                .resource::<GuildState>()
                .info()
                .map(|info| info.guild_id),
            Some(9)
        );
    }

    #[test]
    fn character_selection_clears_and_blocks_same_generation_ingress() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .init_state::<GameState>()
            .add_message::<GuildIngress>()
            .add_message::<ZoneDisconnected>()
            .insert_resource(ZoneSessionGeneration(1))
            .add_plugins(GuildPlugin);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();
        assert!(app.world().resource::<GuildState>().in_guild());

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::CharacterSelection);
        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(8, member(42, 0))),
        );
        app.update();

        assert!(!app.world().resource::<GuildState>().in_guild());
    }

    #[test]
    fn expelled_target_stays_stale_until_character_selection_resets_state() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .init_state::<GameState>()
            .add_message::<GuildIngress>()
            .add_message::<ZoneDisconnected>()
            .insert_resource(ZoneSessionGeneration(1))
            .add_plugins(GuildPlugin);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();

        ingress(
            &mut app,
            1,
            GuildIngressPayload::Info(info(7, member(42, 0))),
        );
        app.update();
        ingress(
            &mut app,
            1,
            GuildIngressPayload::ActionResult(GuildActionResult {
                action: "expel".into(),
                success: true,
                error: GuildErrorKind::None,
            }),
        );
        app.update();
        assert!(app.world().resource::<GuildState>().in_guild());

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::CharacterSelection);
        app.update();

        assert!(!app.world().resource::<GuildState>().in_guild());
    }
}
