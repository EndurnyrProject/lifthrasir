//! Peco Peco mount slash commands and feedback.
//!
//! `chat_input_control` (`chat_box.rs`) calls [`parse_mount_slash`] alongside the
//! emote/party parsers; `/mount` and `/unmount` become a [`MountPeco`] command
//! instead of a chat message. [`ingest_mount_feedback`] echoes server rejections
//! as red chat lines; a success is silent — the body sprite swap is the feedback.

use bevy::prelude::*;
use net_contract::events::{PecoMountRejection, PecoMountResult};

use crate::theme;
use crate::widgets::chat_box::{ChatHistory, append_colored_line};

/// Parse one chat line into a mount intent: `/mount` is `Some(true)`,
/// `/unmount` is `Some(false)`, anything else falls through to the next parser.
pub fn parse_mount_slash(input: &str) -> Option<bool> {
    match input.trim() {
        "/mount" => Some(true),
        "/unmount" => Some(false),
        _ => None,
    }
}

/// A distinct, human-readable line for every [`PecoMountRejection`].
pub fn mount_rejection_text(rejection: PecoMountRejection) -> &'static str {
    match rejection {
        PecoMountRejection::SkillNotLearned => "You have not learned Peco Peco Riding.",
        PecoMountRejection::AlreadyMounted => "You are already riding a Peco Peco.",
        PecoMountRejection::NotMounted => "You are not riding a Peco Peco.",
        PecoMountRejection::Dead => "You cannot do that while dead.",
    }
}

/// Reads [`PecoMountResult`] and echoes each rejection as one red chat line.
pub(crate) fn ingest_mount_feedback(
    mut results: MessageReader<PecoMountResult>,
    container: Query<Entity, With<ChatHistory>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if results.is_empty() {
        return;
    }
    let Ok(container) = container.single() else {
        return;
    };
    let font = asset_server.load(theme::FONT_BODY);

    for event in results.read() {
        let Err(rejection) = event.outcome else {
            continue;
        };
        append_colored_line(
            &mut commands,
            container,
            mount_rejection_text(rejection),
            theme::BAD,
            font.clone(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn parses_mount_and_unmount() {
        assert_eq!(parse_mount_slash("/mount"), Some(true));
        assert_eq!(parse_mount_slash("/unmount"), Some(false));
        assert_eq!(parse_mount_slash("  /mount  "), Some(true));
    }

    #[test]
    fn other_input_falls_through() {
        assert_eq!(parse_mount_slash("hello"), None);
        assert_eq!(parse_mount_slash("/mountain"), None);
        assert_eq!(parse_mount_slash("/pinvite"), None);
    }

    #[test]
    fn rejection_text_is_distinct_and_nonempty_per_variant() {
        let all = [
            PecoMountRejection::SkillNotLearned,
            PecoMountRejection::AlreadyMounted,
            PecoMountRejection::NotMounted,
            PecoMountRejection::Dead,
        ];
        let texts: HashSet<&'static str> = all.iter().copied().map(mount_rejection_text).collect();
        assert_eq!(texts.len(), all.len());
        assert!(texts.iter().all(|text| !text.is_empty()));
    }

    #[test]
    fn ingest_appends_a_line_per_rejection_only() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Font>();
        app.add_message::<PecoMountResult>();
        app.world_mut().spawn(ChatHistory);
        app.add_systems(Update, ingest_mount_feedback);

        app.world_mut()
            .resource_mut::<Messages<PecoMountResult>>()
            .write(PecoMountResult { outcome: Ok(()) });
        app.world_mut()
            .resource_mut::<Messages<PecoMountResult>>()
            .write(PecoMountResult {
                outcome: Err(PecoMountRejection::SkillNotLearned),
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
        assert_eq!(lines, 1);
    }
}
