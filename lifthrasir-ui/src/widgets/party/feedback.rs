//! Party results and lifecycle transitions as colored chat lines, mirroring
//! `announcement.rs::ingest_announcements`.
//!
//! [`PartyActionResulted`] and [`PartyDisbanded`] are UI-only concerns (the roster
//! itself is driven solely by `PartyInfoReceived`/`PartyDisbanded` in `game-engine`'s
//! `PartyState`), so this module owns their entire rendering: a per-`PartyErrorKind`
//! failure table, a per-action success table, and a disband line.

use bevy::prelude::*;
use net_contract::dto::PartyErrorKind;
use net_contract::events::{PartyActionResulted, PartyDisbanded};

use crate::theme;
use crate::widgets::chat_box::{ChatHistory, append_colored_line};

/// A distinct, human-readable line for every `PartyErrorKind`. `None` shouldn't occur
/// on a `success:false` result, but still gets a generic fallback rather than a panic.
pub fn party_error_text(error: PartyErrorKind) -> &'static str {
    match error {
        PartyErrorKind::None => "Party action failed.",
        PartyErrorKind::NameTaken => "That party name is already taken.",
        PartyErrorKind::AlreadyInParty => "You are already in a party.",
        PartyErrorKind::PartyFull => "The party is full.",
        PartyErrorKind::NotLeader => "Only the party leader can do that.",
        PartyErrorKind::LevelRange => "That player is outside the party's level range.",
        PartyErrorKind::SameAccount => "You cannot invite a character on your own account.",
        PartyErrorKind::TargetOffline => "That player is offline.",
        PartyErrorKind::NotMember => "That player is not in your party.",
        PartyErrorKind::NotSameMap => "That player must be on the same map.",
    }
}

/// A confirming line for a successful `PartyActionResulted`, keyed by aesir's exact
/// `action` string (`party_handler.ex`: `create`, `invite`, `invite_response`,
/// `leave`). An unrecognized action still gets a generic confirmation.
pub fn party_success_text(action: &str) -> String {
    match action {
        "create" => "Party created.".to_string(),
        "invite" => "Invite sent.".to_string(),
        "invite_response" => "You joined the party.".to_string(),
        "leave" => "You left the party.".to_string(),
        _ => "Party action succeeded.".to_string(),
    }
}

/// The disband line: the reason in parentheses when the server supplied one.
fn disband_text(reason: &str) -> String {
    if reason.is_empty() {
        "The party has disbanded.".to_string()
    } else {
        format!("The party has disbanded. ({reason})")
    }
}

/// Reads `PartyActionResulted` and `PartyDisbanded` and echoes each as one chat line:
/// green for a success, red for a failure (`party_error_text`), amber for a disband.
pub(crate) fn ingest_party_feedback(
    mut results: MessageReader<PartyActionResulted>,
    mut disbanded: MessageReader<PartyDisbanded>,
    container: Query<Entity, With<ChatHistory>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if results.is_empty() && disbanded.is_empty() {
        return;
    }
    let Ok(container) = container.single() else {
        return;
    };
    let font = asset_server.load(theme::FONT_BODY);

    for event in results.read() {
        let (text, color) = if event.success {
            (party_success_text(&event.action), theme::EMERALD)
        } else {
            (party_error_text(event.error).to_string(), theme::BAD)
        };
        append_colored_line(&mut commands, container, &text, color, font.clone());
    }

    for event in disbanded.read() {
        append_colored_line(
            &mut commands,
            container,
            &disband_text(&event.reason),
            theme::WARN,
            font.clone(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const ALL_ERRORS: [PartyErrorKind; 10] = [
        PartyErrorKind::None,
        PartyErrorKind::NameTaken,
        PartyErrorKind::AlreadyInParty,
        PartyErrorKind::PartyFull,
        PartyErrorKind::NotLeader,
        PartyErrorKind::LevelRange,
        PartyErrorKind::SameAccount,
        PartyErrorKind::TargetOffline,
        PartyErrorKind::NotMember,
        PartyErrorKind::NotSameMap,
    ];

    #[test]
    fn party_error_text_is_distinct_and_nonempty_per_variant() {
        let texts: HashSet<&'static str> =
            ALL_ERRORS.iter().copied().map(party_error_text).collect();
        assert_eq!(texts.len(), ALL_ERRORS.len());
        assert!(texts.iter().all(|text| !text.is_empty()));
    }

    #[test]
    fn party_success_text_maps_known_actions() {
        assert_eq!(party_success_text("create"), "Party created.");
        assert_eq!(party_success_text("invite"), "Invite sent.");
        assert_eq!(
            party_success_text("invite_response"),
            "You joined the party."
        );
        assert_eq!(party_success_text("leave"), "You left the party.");
    }

    #[test]
    fn party_success_text_falls_back_for_unknown_action() {
        assert_eq!(party_success_text("kick"), "Party action succeeded.");
    }

    #[test]
    fn disband_text_includes_reason_only_when_present() {
        assert_eq!(disband_text(""), "The party has disbanded.");
        assert_eq!(
            disband_text("leader left"),
            "The party has disbanded. (leader left)"
        );
    }

    fn ingest_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Font>();
        app.add_message::<PartyActionResulted>();
        app.add_message::<PartyDisbanded>();
        app.world_mut().spawn(ChatHistory);
        app.add_systems(Update, ingest_party_feedback);
        app
    }

    #[test]
    fn ingest_appends_a_line_per_event() {
        let mut app = ingest_app();
        app.world_mut()
            .resource_mut::<Messages<PartyActionResulted>>()
            .write(PartyActionResulted {
                action: "create".to_string(),
                success: true,
                error: PartyErrorKind::None,
            });
        app.world_mut()
            .resource_mut::<Messages<PartyActionResulted>>()
            .write(PartyActionResulted {
                action: "invite".to_string(),
                success: false,
                error: PartyErrorKind::NotLeader,
            });
        app.world_mut()
            .resource_mut::<Messages<PartyDisbanded>>()
            .write(PartyDisbanded {
                party_id: 1,
                reason: String::new(),
            });
        app.update();

        let container = app
            .world_mut()
            .query_filtered::<Entity, With<ChatHistory>>()
            .single(app.world())
            .unwrap();
        let lines = app
            .world()
            .get::<Children>(container)
            .map(|c| c.len())
            .unwrap_or(0);
        assert_eq!(lines, 3);
    }
}
