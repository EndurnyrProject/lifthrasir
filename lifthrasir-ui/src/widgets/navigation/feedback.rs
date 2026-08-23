use bevy::prelude::*;
use net_contract::dto::{NavigationEnd, NavigationFailure};
use net_contract::events::{NavigationEnded, NavigationFailed};

use crate::theme;
use crate::widgets::chat_box::{ChatHistory, append_colored_line};

pub(crate) fn ingest_navigation_feedback(
    mut failed: MessageReader<NavigationFailed>,
    mut ended: MessageReader<NavigationEnded>,
    container: Query<Entity, With<ChatHistory>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if failed.is_empty() && ended.is_empty() {
        return;
    }
    let Ok(container) = container.single() else {
        return;
    };
    let font = asset_server.load(theme::FONT_BODY);

    for event in failed.read() {
        let text = match event.reason {
            NavigationFailure::Unresolved => "Navigation target could not be found.",
            NavigationFailure::Unreachable => "No route exists to that destination.",
            NavigationFailure::AlreadyThere => "You are already on that map.",
            NavigationFailure::Excluded => "That destination is excluded from navigation.",
        };
        append_colored_line(&mut commands, container, text, theme::BAD, font.clone());
    }

    for event in ended.read() {
        let (text, color) = match event.reason {
            NavigationEnd::Arrived => ("You have arrived at your destination.", theme::EMERALD),
            NavigationEnd::Cancelled => ("Navigation cancelled.", theme::WARN),
        };
        append_colored_line(&mut commands, container, text, color, font.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::{NavigationEnd, NavigationFailure};

    const ALL_FAILURES: [NavigationFailure; 4] = [
        NavigationFailure::Unresolved,
        NavigationFailure::Unreachable,
        NavigationFailure::AlreadyThere,
        NavigationFailure::Excluded,
    ];

    fn ingest_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Font>();
        app.add_message::<NavigationFailed>();
        app.add_message::<NavigationEnded>();
        app.world_mut().spawn(ChatHistory);
        app.add_systems(Update, ingest_navigation_feedback);
        app
    }

    fn chat_lines(app: &mut App) -> Vec<String> {
        let container = app
            .world_mut()
            .query_filtered::<Entity, With<ChatHistory>>()
            .single(app.world())
            .unwrap();
        app.world()
            .get::<Children>(container)
            .into_iter()
            .flatten()
            .filter_map(|line| app.world().get::<Text>(*line))
            .map(|text| text.0.clone())
            .collect()
    }

    #[test]
    fn failures_append_distinct_lines() {
        let mut app = ingest_app();
        for reason in ALL_FAILURES {
            app.world_mut().write_message(NavigationFailed { reason });
        }
        app.update();

        let lines = chat_lines(&mut app);
        // The exact ordered comparison below already proves count, uniqueness,
        // wording, ordering, and that all four same-frame messages were processed.
        assert_eq!(
            lines,
            [
                "Navigation target could not be found.",
                "No route exists to that destination.",
                "You are already on that map.",
                "That destination is excluded from navigation.",
            ]
        );
    }

    #[test]
    fn ends_append_distinct_lines() {
        let mut app = ingest_app();
        app.world_mut().write_message(NavigationEnded {
            reason: NavigationEnd::Arrived,
        });
        app.world_mut().write_message(NavigationEnded {
            reason: NavigationEnd::Cancelled,
        });
        app.update();

        let lines = chat_lines(&mut app);
        assert_eq!(lines.len(), 2);
        assert_ne!(lines[0], lines[1]);
    }

    #[test]
    fn missing_chat_history_is_a_no_op() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Font>();
        app.add_message::<NavigationFailed>();
        app.add_message::<NavigationEnded>();
        app.add_systems(Update, ingest_navigation_feedback);
        app.world_mut().write_message(NavigationFailed {
            reason: NavigationFailure::Unresolved,
        });
        app.world_mut().write_message(NavigationEnded {
            reason: NavigationEnd::Arrived,
        });

        app.update();
    }
}
