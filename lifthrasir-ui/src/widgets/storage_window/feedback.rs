//! Storage errors without a pending transfer must remain visible outside the vault window.

use bevy::prelude::*;
use game_engine::domain::storage::Storage;
use net_contract::events::StorageResult;

use super::{StorageUi, rejection_message};
use crate::theme;
use crate::widgets::chat_box::{ChatHistory, append_colored_line};

/// Opening failures have no window in which to display panel feedback.
/// Read before the transfer handler clears `awaiting_result` to avoid duplicate feedback.
pub(super) fn ingest_unprompted_errors(
    mut results: MessageReader<StorageResult>,
    storage: Res<Storage>,
    ui: Res<StorageUi>,
    history: Query<Entity, With<ChatHistory>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    for result in results.read() {
        if storage.is_open() && ui.awaiting_result {
            continue;
        }
        let Err(rejection) = result.outcome else {
            continue;
        };
        let text = rejection_message(rejection);
        let Ok(history) = history.single() else {
            warn!("{text}");
            continue;
        };
        append_colored_line(
            &mut commands,
            history,
            &text,
            theme::BAD,
            assets.load(theme::FONT_BODY),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::apply_storage_results;
    use super::*;
    use net_contract::dto::StorageKind;
    use net_contract::events::StorageRejection;

    #[test]
    fn opening_rejections_are_visible_once_without_a_storage_window() {
        let cases = [
            (StorageRejection::NoGuild, "You are not in a guild."),
            (
                StorageRejection::GuildNoSkill,
                "Your guild needs the Guild Storage skill.",
            ),
            (
                StorageRejection::GuildNoPermission,
                "You do not have permission to use guild storage.",
            ),
            (
                StorageRejection::GuildInUse,
                "Guild storage is already in use.",
            ),
            (
                StorageRejection::OtherStorageOpen,
                "Close your other storage window first.",
            ),
            (
                StorageRejection::Rental,
                "Rental items cannot be stored here.",
            ),
            (
                StorageRejection::NoGuildStorage,
                "Guild storage is unavailable.",
            ),
            (
                StorageRejection::Stale,
                "Your storage session has expired. Reopen storage.",
            ),
        ];
        for (rejection, expected) in cases {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default()));
            app.init_asset::<Font>();
            app.init_resource::<Storage>();
            app.init_resource::<StorageUi>();
            app.add_message::<StorageResult>();
            app.world_mut().spawn(ChatHistory);
            app.add_systems(
                Update,
                (ingest_unprompted_errors, apply_storage_results).chain(),
            );
            app.world_mut().write_message(StorageResult {
                outcome: Err(rejection),
            });
            app.update();
            let mut lines = app.world_mut().query::<(&Text, &TextColor)>();
            let (text, color) = lines.single(app.world()).unwrap();
            assert_eq!(text.0, expected);
            assert_eq!(color.0, theme::BAD);
            assert!(!app.world().resource::<Storage>().is_open());
            app.update();
            assert_eq!(lines.iter(app.world()).count(), 1);

            app.world_mut()
                .resource_mut::<Storage>()
                .open(StorageKind::Guild, 600, vec![]);
            app.world_mut().resource_mut::<StorageUi>().awaiting_result = true;
            app.world_mut().write_message(StorageResult {
                outcome: Err(rejection),
            });
            app.update();
            assert_eq!(
                lines.iter(app.world()).count(),
                1,
                "pending transfers use panel feedback, not duplicate chat"
            );
            assert_eq!(
                app.world().resource::<StorageUi>().panel_error.as_deref(),
                Some(expected)
            );
            assert!(!app.world().resource::<StorageUi>().awaiting_result);
        }
    }
}
