use bevy::prelude::*;
use game_engine::domain::guild::GuildState;
use net_contract::dto::GuildErrorKind;
use net_contract::events::{GuildIngress, GuildIngressPayload};
use net_contract::state::ZoneSessionGeneration;

use super::{GuildUi, GuildUiSession, emblem};
use crate::theme;
use crate::widgets::chat_box::{ChatHistory, append_colored_line};

pub(super) fn apply_guild_results(
    mut ingress: MessageReader<GuildIngress>,
    generation: Res<ZoneSessionGeneration>,
    session: Option<Res<GuildUiSession>>,
    mut ui: ResMut<GuildUi>,
    mut images: ResMut<emblem::GuildEmblemPreview>,
    mut assets: ResMut<Assets<Image>>,
) {
    if session.as_deref().is_some_and(|session| session.blocked) {
        ingress.clear();
        return;
    }
    for event in ingress.read() {
        let GuildIngressPayload::ActionResult(result) = &event.payload else {
            continue;
        };
        if result.action == "alliance_request"
            && !result.success
            && result.error == GuildErrorKind::AllianceDeclined
        {
            continue;
        }
        let matches = ui.pending.as_ref().is_some_and(|pending| {
            pending.action == result.action
                && pending.generation == event.generation
                && event.generation == *generation
        });
        if !matches {
            if let Some(pending) = ui.pending.as_ref()
                && pending.action != result.action
            {
                warn!(
                    expected = pending.action,
                    received = %result.action,
                    "ignoring mismatched guild action result"
                );
            }
            continue;
        }
        ui.pending = None;
        if result.success {
            ui.feedback = Some(guild_success_text(&result.action).to_string());
            ui.feedback_is_error = false;
        } else {
            if result.action == "emblem_upload" {
                images.discard_preview(&mut assets);
            }
            ui.feedback = Some(guild_action_error_text(&result.action, result.error).to_string());
            ui.feedback_is_error = true;
        }
    }
}

/// Short confirmation shown once the server accepts `action`. The guild snapshot
/// that follows refreshes the window on its own, so the text never mentions it.
pub(super) fn guild_success_text(action: &str) -> &'static str {
    match action {
        "create" => "Guild created",
        "invite" => "Invitation sent",
        "position_edit" => "Position saved",
        "member_position" => "Position assigned",
        "notice_edit" => "Notice saved",
        "emblem_upload" => "Emblem updated",
        "leave" => "You left the guild",
        "expel" => "Member expelled",
        "skill_up" => "Guild skill upgraded",
        "alliance_request" => "Alliance request sent",
        "alliance_response" => "Alliance response sent",
        "alliance_break" => "Alliance broken",
        "antagonist" => "Antagonist declared",
        "antagonist_remove" => "Antagonist removed",
        _ => "Done",
    }
}

pub(super) fn guild_error_text(error: GuildErrorKind) -> &'static str {
    match error {
        GuildErrorKind::None => "Success",
        GuildErrorKind::NameTaken => "Guild name is already taken",
        GuildErrorKind::AlreadyInGuild => "Character already belongs to a guild",
        GuildErrorKind::GuildFull => "Guild is full",
        GuildErrorKind::NoPermission => "Current position lacks permission",
        GuildErrorKind::NotMember => "Character is not a guild member",
        GuildErrorKind::TargetOffline => "Target is offline",
        GuildErrorKind::NoEmperium => "Creation requires an Emperium",
        GuildErrorKind::InvalidEmblem => "Emblem is invalid",
        GuildErrorKind::CannotTargetMaster => "Guild master cannot be expelled",
        GuildErrorKind::InvalidPosition => "Position is invalid",
        GuildErrorKind::NoSkillPoints => "No guild skill points available",
        GuildErrorKind::SkillRequirement => "Guild skill requirements are not met",
        GuildErrorKind::SkillMaxed => "Guild skill is already at maximum level",
        GuildErrorKind::AllyLimit => "Guild alliance limit reached",
        GuildErrorKind::AntagonistLimit => "Guild antagonist limit reached",
        GuildErrorKind::AlreadyAllied => "Guilds are already allied",
        GuildErrorKind::AlreadyAntagonist => "Guild is already an antagonist",
        GuildErrorKind::NotRelated => "Guild relation does not exist",
        GuildErrorKind::SameGuild => "Cannot target the same guild",
        GuildErrorKind::SiegeActive => "Guild relations cannot change during a siege",
        GuildErrorKind::RequestPending => "An alliance request is already pending",
        GuildErrorKind::AllianceDeclined => "Alliance request was declined",
        GuildErrorKind::Unknown(value) => {
            warn!(value, "unknown guild operation error");
            "Guild operation failed"
        }
    }
}

pub(super) fn guild_action_error_text(action: &str, error: GuildErrorKind) -> &'static str {
    match (action, error) {
        (_, GuildErrorKind::None) => "Guild action failed",
        (
            "skill_up" | "alliance_request" | "alliance_response" | "alliance_break" | "antagonist"
            | "antagonist_remove",
            GuildErrorKind::NoPermission,
        ) => "Only the guild master can do that",
        (
            "alliance_request" | "alliance_response" | "alliance_break",
            GuildErrorKind::SiegeActive,
        ) => "This alliance action is unavailable during the current siege",
        ("antagonist" | "antagonist_remove", GuildErrorKind::SiegeActive) => {
            "This antagonist action was rejected during the current siege"
        }
        (_, GuildErrorKind::SiegeActive) => {
            "This guild action was rejected during the current siege"
        }
        (_, error) => guild_error_text(error),
    }
}

pub(super) fn ingest_guild_announcements(
    mut ingress: MessageReader<GuildIngress>,
    generation: Res<ZoneSessionGeneration>,
    session: Res<GuildUiSession>,
    guild: Res<GuildState>,
    container: Query<Entity, With<ChatHistory>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if session.blocked || session.generation != *generation {
        ingress.clear();
        return;
    }
    if ingress.is_empty() {
        return;
    }
    let Ok(container) = container.single() else {
        return;
    };
    let font = asset_server.load(theme::FONT_BODY);
    let guild_id = guild.info().map(|info| info.guild_id);

    for event in ingress.read() {
        if event.generation != *generation {
            continue;
        }
        match &event.payload {
            GuildIngressPayload::ActionResult(result) if result.action == "create" => {
                let (text, color) = if result.success {
                    (guild_success_text("create"), theme::GOLD)
                } else {
                    (guild_action_error_text("create", result.error), theme::BAD)
                };
                append_colored_line(&mut commands, container, text, color, font.clone());
            }
            GuildIngressPayload::ActionResult(result)
                if result.action == "alliance_request"
                    && !result.success
                    && result.error == GuildErrorKind::AllianceDeclined =>
            {
                append_colored_line(
                    &mut commands,
                    container,
                    "Alliance request declined or expired.",
                    theme::BAD,
                    font.clone(),
                );
            }
            GuildIngressPayload::LevelUp {
                guild_id: event_guild_id,
                level,
                skill_points,
            } if guild_id == Some(*event_guild_id) => {
                let point_label = if *skill_points == 1 {
                    "skill point"
                } else {
                    "skill points"
                };
                let text =
                    format!("Guild reached level {level}! {skill_points} {point_label} available.");
                append_colored_line(&mut commands, container, &text, theme::GOLD, font.clone());
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::chat_box::ChatHistory;
    use crate::widgets::guild_window::{
        GuildUi, GuildUiSession, GuildWindowRoot, PendingGuildMutation,
    };
    use game_engine::domain::guild::{GuildPlugin, GuildSystems};
    use net_contract::dto::{
        GuildActionResult, GuildErrorKind, GuildInfo, GuildMemberInfo, GuildPositionInfo,
    };
    use net_contract::events::{GuildIngress, GuildIngressPayload, ZoneDisconnected};
    use net_contract::state::ZoneSessionGeneration;

    fn guild_info(guild_id: u32) -> GuildInfo {
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
                can_storage: true,
                tax: 0,
            }],
            members: vec![GuildMemberInfo {
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
            }],
            level: 1,
            exp: 0,
            next_exp: 100,
            skill_points: 0,
            skills: vec![],
            relations: vec![],
        }
    }

    fn feedback_app(generation: ZoneSessionGeneration) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Font>();
        app.add_message::<GuildIngress>()
            .add_message::<ZoneDisconnected>()
            .insert_resource(generation)
            .insert_resource(GuildUiSession {
                generation,
                ..default()
            })
            .init_resource::<GuildUi>()
            .init_resource::<emblem::GuildEmblemPreview>()
            .add_plugins(GuildPlugin);
        app.world_mut().spawn(ChatHistory);
        app.world_mut().spawn((GuildWindowRoot, Visibility::Hidden));
        app.add_systems(
            Update,
            (apply_guild_results, ingest_guild_announcements)
                .chain()
                .in_set(GuildSystems::UiSync),
        );
        app
    }

    #[test]
    fn creation_results_appear_in_chat_with_the_guild_window_closed() {
        let generation = ZoneSessionGeneration(9);
        for (success, error, expected) in [
            (true, GuildErrorKind::None, "Guild created"),
            (
                false,
                GuildErrorKind::NoEmperium,
                "Creation requires an Emperium",
            ),
            (
                false,
                GuildErrorKind::NameTaken,
                "Guild name is already taken",
            ),
        ] {
            let mut app = feedback_app(generation);
            app.world_mut().resource_mut::<GuildUi>().pending = Some(PendingGuildMutation {
                action: "create",
                generation,
            });
            for event_generation in [ZoneSessionGeneration(8), generation] {
                app.world_mut().write_message(GuildIngress {
                    generation: event_generation,
                    payload: GuildIngressPayload::ActionResult(GuildActionResult {
                        action: "create".into(),
                        success,
                        error,
                    }),
                });
            }
            app.update();

            let mut lines = app.world_mut().query::<&Text>();
            assert_eq!(lines.single(app.world()).unwrap().0, expected);
            assert!(app.world().resource::<GuildUi>().pending.is_none());
            assert!(!app.world().resource::<GuildState>().in_guild());
            assert_eq!(
                *app.world_mut()
                    .query_filtered::<&Visibility, With<GuildWindowRoot>>()
                    .single(app.world())
                    .unwrap(),
                Visibility::Hidden
            );
        }
    }

    #[test]
    fn late_alliance_decline_announces_without_a_pending_action() {
        let generation = ZoneSessionGeneration(9);
        let mut app = feedback_app(generation);
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::ActionResult(GuildActionResult {
                action: "alliance_request".into(),
                success: false,
                error: GuildErrorKind::AllianceDeclined,
            }),
        });

        app.update();

        let mut lines = app.world_mut().query::<&Text>();
        assert_eq!(
            lines.single(app.world()).unwrap().0,
            "Alliance request declined or expired."
        );
    }

    #[test]
    fn late_alliance_decline_does_not_clear_a_different_pending_action() {
        let generation = ZoneSessionGeneration(9);
        let mut app = feedback_app(generation);
        app.world_mut().resource_mut::<GuildUi>().pending = Some(PendingGuildMutation {
            action: "skill_up",
            generation,
        });
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::ActionResult(GuildActionResult {
                action: "alliance_request".into(),
                success: false,
                error: GuildErrorKind::AllianceDeclined,
            }),
        });

        app.update();

        assert_eq!(
            app.world().resource::<GuildUi>().pending,
            Some(PendingGuildMutation {
                action: "skill_up",
                generation,
            })
        );
        let mut lines = app.world_mut().query::<&Text>();
        assert_eq!(
            lines.single(app.world()).unwrap().0,
            "Alliance request declined or expired."
        );
    }

    #[test]
    fn late_alliance_decline_does_not_clear_a_newer_alliance_request() {
        let generation = ZoneSessionGeneration(9);
        let mut app = feedback_app(generation);
        app.world_mut().resource_mut::<GuildUi>().pending = Some(PendingGuildMutation {
            action: "alliance_request",
            generation,
        });
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::ActionResult(GuildActionResult {
                action: "alliance_request".into(),
                success: false,
                error: GuildErrorKind::AllianceDeclined,
            }),
        });

        app.update();

        assert_eq!(
            app.world().resource::<GuildUi>().pending,
            Some(PendingGuildMutation {
                action: "alliance_request",
                generation,
            })
        );
        let mut lines = app.world_mut().query::<&Text>();
        assert_eq!(
            lines.single(app.world()).unwrap().0,
            "Alliance request declined or expired."
        );
    }

    #[test]
    fn stale_matching_action_result_does_not_clear_current_pending() {
        let generation = ZoneSessionGeneration(9);
        let mut app = feedback_app(generation);
        app.world_mut().resource_mut::<GuildUi>().pending = Some(PendingGuildMutation {
            action: "skill_up",
            generation,
        });
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(8),
            payload: GuildIngressPayload::ActionResult(GuildActionResult {
                action: "skill_up".into(),
                success: false,
                error: GuildErrorKind::NoSkillPoints,
            }),
        });

        app.update();

        assert_eq!(
            app.world().resource::<GuildUi>().pending,
            Some(PendingGuildMutation {
                action: "skill_up",
                generation,
            })
        );
    }

    #[test]
    fn matching_level_up_announces_while_guild_window_is_hidden() {
        let generation = ZoneSessionGeneration(9);
        let mut app = feedback_app(generation);
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::Info(guild_info(7)),
        });
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::LevelUp {
                guild_id: 7,
                level: 2,
                skill_points: 1,
            },
        });

        app.update();

        let mut lines = app.world_mut().query::<&Text>();
        assert_eq!(
            lines.single(app.world()).unwrap().0,
            "Guild reached level 2! 1 skill point available."
        );
        let mut roots = app
            .world_mut()
            .query_filtered::<&Visibility, With<GuildWindowRoot>>();
        assert_eq!(*roots.single(app.world()).unwrap(), Visibility::Hidden);

        app.update();

        assert_eq!(lines.iter(app.world()).count(), 1);
    }

    #[test]
    fn stale_and_other_guild_level_ups_do_not_announce() {
        let generation = ZoneSessionGeneration(9);
        let mut app = feedback_app(generation);
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::Info(guild_info(7)),
        });
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(8),
            payload: GuildIngressPayload::LevelUp {
                guild_id: 7,
                level: 2,
                skill_points: 1,
            },
        });
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::LevelUp {
                guild_id: 8,
                level: 2,
                skill_points: 1,
            },
        });

        app.update();

        assert_eq!(
            app.world_mut().query::<&Text>().iter(app.world()).count(),
            0
        );
    }

    #[test]
    fn blocked_session_discards_level_up_notifications() {
        let generation = ZoneSessionGeneration(9);
        let mut app = feedback_app(generation);
        app.world_mut().resource_mut::<GuildUiSession>().blocked = true;
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::LevelUp {
                guild_id: 7,
                level: 2,
                skill_points: 1,
            },
        });

        app.update();

        assert_eq!(
            app.world_mut().query::<&Text>().iter(app.world()).count(),
            0
        );
    }

    #[test]
    fn new_action_results_release_matching_pending_with_accurate_feedback() {
        let generation = ZoneSessionGeneration(9);
        for (action, expected) in [
            ("skill_up", "Guild skill upgraded"),
            ("alliance_request", "Alliance request sent"),
            ("alliance_response", "Alliance response sent"),
            ("alliance_break", "Alliance broken"),
            ("antagonist", "Antagonist declared"),
            ("antagonist_remove", "Antagonist removed"),
        ] {
            let mut app = feedback_app(generation);
            app.world_mut().resource_mut::<GuildUi>().pending =
                Some(PendingGuildMutation { action, generation });
            app.world_mut().write_message(GuildIngress {
                generation,
                payload: GuildIngressPayload::ActionResult(GuildActionResult {
                    action: action.into(),
                    success: true,
                    error: GuildErrorKind::None,
                }),
            });

            app.update();

            let ui = app.world().resource::<GuildUi>();
            assert!(ui.pending.is_none(), "{action} remained pending");
            assert_eq!(ui.feedback.as_deref(), Some(expected), "{action}");
            assert!(!expected.contains("formed"), "{action}");
        }
    }

    #[test]
    fn action_error_text_uses_operational_permission_and_siege_wording() {
        assert_eq!(
            guild_action_error_text("skill_up", GuildErrorKind::NoPermission),
            "Only the guild master can do that"
        );
        assert_eq!(
            guild_action_error_text("position_edit", GuildErrorKind::NoPermission),
            "Current position lacks permission"
        );
        assert_eq!(
            guild_action_error_text("alliance_request", GuildErrorKind::SiegeActive),
            "This alliance action is unavailable during the current siege"
        );
        assert_eq!(
            guild_action_error_text("antagonist", GuildErrorKind::SiegeActive),
            "This antagonist action was rejected during the current siege"
        );
        assert_eq!(
            guild_action_error_text("skill_up", GuildErrorKind::SkillRequirement),
            "Guild skill requirements are not met"
        );
        assert_eq!(
            guild_action_error_text("skill_up", GuildErrorKind::None),
            "Guild action failed"
        );
    }
}
