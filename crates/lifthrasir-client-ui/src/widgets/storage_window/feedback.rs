//! Storage errors without a pending transfer must remain visible outside the vault window.

use bevy::prelude::*;
use lifthrasir_client::domain::storage::Storage;
use net_contract::events::StorageResult;

use crate::widgets::chat_box::{ChatHistory, append_colored_line};
use crate::widgets::storage_window::{StorageUi, rejection_message};

/// Opening failures have no window in which to display a toast.
/// Read before the transfer handler settles the transfer in flight, to avoid duplicate feedback.
pub(super) fn ingest_unprompted_errors(
    mut results: MessageReader<StorageResult>,
    storage: Res<Storage>,
    ui: Res<StorageUi>,
    history: Query<Entity, With<ChatHistory>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    for result in results.read() {
        if storage.is_open() && ui.in_flight().is_some() {
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
            lifthrasir_ui::theme::BAD,
            assets.load(lifthrasir_ui::theme::FONT_BODY),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{flying, wire_app};
    use super::super::{StorageSelection, drive_transfers};
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
            let mut app = wire_app();
            app.add_plugins((MinimalPlugins, AssetPlugin::default()));
            app.init_asset::<Font>();
            app.world_mut().spawn(ChatHistory);
            app.add_systems(Update, ingest_unprompted_errors.before(drive_transfers));
            app.world_mut().write_message(StorageResult {
                outcome: Err(rejection),
            });
            app.update();
            let mut lines = app.world_mut().query::<(&Text, &TextColor)>();
            let (text, color) = lines.single(app.world()).unwrap();
            assert_eq!(text.0, expected);
            assert_eq!(color.0, lifthrasir_ui::theme::BAD);
            assert!(!app.world().resource::<Storage>().is_open());
            app.update();
            assert_eq!(lines.iter(app.world()).count(), 1);

            app.world_mut()
                .resource_mut::<Storage>()
                .open(StorageKind::Guild, 600, vec![]);
            app.world_mut().resource_mut::<StorageUi>().run =
                Some(flying(StorageSelection::Bag(2), 1));
            app.world_mut().write_message(StorageResult {
                outcome: Err(rejection),
            });
            app.update();
            assert_eq!(
                lines.iter(app.world()).count(),
                1,
                "a transfer in flight is answered by the toast, not duplicate chat"
            );
            let ui = app.world().resource::<StorageUi>();
            let toast = ui.toast.as_ref().expect("the rejection is toasted");
            assert_eq!((toast.text.as_str(), toast.bad), (expected, true));
            assert!(!ui.busy());
        }
    }
}
