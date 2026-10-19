use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::storage::{
    storage_item_added, storage_item_removed, storage_opened, storage_result,
};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{StorageItemAdded, StorageItemRemoved, StorageOpened, StorageResult};

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_storage(
    mut incoming: MessageReader<IncomingMessage>,
    mut opened: MessageWriter<StorageOpened>,
    mut added: MessageWriter<StorageItemAdded>,
    mut removed: MessageWriter<StorageItemRemoved>,
    mut result: MessageWriter<StorageResult>,
) {
    for message in incoming.read() {
        match message.body.clone() {
            Body::StorageOpened(snapshot) => {
                if let Some(snapshot) = storage_opened(snapshot) {
                    opened.write(snapshot);
                }
            }
            Body::StorageItemAdded(delta) => {
                if let Some(delta) = storage_item_added(delta) {
                    added.write(delta);
                }
            }
            Body::StorageItemRemoved(delta) => {
                if let Some(delta) = storage_item_removed(delta) {
                    removed.write(delta);
                }
            }
            Body::StorageResult(outcome) => {
                result.write(storage_result(outcome));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::{BULK, GAMEPLAY};
    use crate::proto::aesir::net;

    fn drain(bodies: Vec<(u8, Body)>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<StorageOpened>()
            .add_message::<StorageItemAdded>()
            .add_message::<StorageItemRemoved>()
            .add_message::<StorageResult>()
            .add_systems(Update, zone_drain_storage);

        for (channel, body) in bodies {
            app.world_mut()
                .write_message(IncomingMessage { channel, body });
        }
        app.update();
        app
    }

    #[test]
    fn storage_opened_body_produces_one_snapshot() {
        let app = drain(vec![(
            BULK,
            Body::StorageOpened(net::StorageOpened {
                capacity: 600,
                items: vec![net::InventoryItem {
                    index: 70_000,
                    amount: 80_000,
                    weight: 10,
                    ..Default::default()
                }],
                kind: net::StorageKind::Personal as i32,
            }),
        )]);

        let messages = app.world().resource::<Messages<StorageOpened>>();
        let snapshots: Vec<_> = messages.iter_current_update_messages().collect();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].capacity, 600);
        assert_eq!(snapshots[0].items[0].index, 70_000);
        assert_eq!(snapshots[0].items[0].amount, 80_000);
    }

    #[test]
    fn storage_item_added_body_produces_one_delta() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::StorageItemAdded(net::StorageItemAdded {
                index: 70_001,
                amount: 80_001,
                weight: 12,
                ..Default::default()
            }),
        )]);

        let messages = app.world().resource::<Messages<StorageItemAdded>>();
        let deltas: Vec<_> = messages.iter_current_update_messages().collect();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].item.index, 70_001);
        assert_eq!(deltas[0].item.amount, 80_001);
        assert_eq!(deltas[0].item.weight, 12);
    }

    #[test]
    fn storage_item_removed_body_produces_one_delta() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::StorageItemRemoved(net::StorageItemRemoved {
                index: 70_002,
                amount: 80_002,
                reason: 4,
                kind: net::StorageKind::Personal as i32,
            }),
        )]);

        let messages = app.world().resource::<Messages<StorageItemRemoved>>();
        let deltas: Vec<_> = messages.iter_current_update_messages().collect();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].index, 70_002);
        assert_eq!(deltas[0].amount, 80_002);
        assert_eq!(deltas[0].reason, 4);
    }

    #[test]
    fn storage_result_body_produces_one_typed_result() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::StorageResult(net::StorageResult {
                result: net::StorageResultCode::StorageNotOpen as i32,
            }),
        )]);

        let messages = app.world().resource::<Messages<StorageResult>>();
        let results: Vec<_> = messages.iter_current_update_messages().collect();
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].outcome,
            Err(net_contract::events::StorageRejection::NotOpen)
        );
    }

    #[test]
    fn storage_snapshots_and_deltas_preserve_personal_and_guild_kind() {
        for (wire, expected) in [
            (
                net::StorageKind::Personal,
                net_contract::dto::StorageKind::Personal,
            ),
            (
                net::StorageKind::Guild,
                net_contract::dto::StorageKind::Guild,
            ),
        ] {
            let app = drain(vec![
                (
                    BULK,
                    Body::StorageOpened(net::StorageOpened {
                        kind: wire as i32,
                        ..Default::default()
                    }),
                ),
                (
                    GAMEPLAY,
                    Body::StorageItemAdded(net::StorageItemAdded {
                        kind: wire as i32,
                        ..Default::default()
                    }),
                ),
                (
                    GAMEPLAY,
                    Body::StorageItemRemoved(net::StorageItemRemoved {
                        kind: wire as i32,
                        ..Default::default()
                    }),
                ),
            ]);
            assert_eq!(
                app.world()
                    .resource::<Messages<StorageOpened>>()
                    .iter_current_update_messages()
                    .next()
                    .unwrap()
                    .kind,
                expected
            );
            assert_eq!(
                app.world()
                    .resource::<Messages<StorageItemAdded>>()
                    .iter_current_update_messages()
                    .next()
                    .unwrap()
                    .kind,
                expected
            );
            assert_eq!(
                app.world()
                    .resource::<Messages<StorageItemRemoved>>()
                    .iter_current_update_messages()
                    .next()
                    .unwrap()
                    .kind,
                expected
            );
        }
    }

    #[test]
    fn unknown_storage_kind_never_opens_or_changes_a_vault() {
        let app = drain(vec![
            (
                BULK,
                Body::StorageOpened(net::StorageOpened {
                    kind: 99,
                    ..Default::default()
                }),
            ),
            (
                GAMEPLAY,
                Body::StorageItemAdded(net::StorageItemAdded {
                    kind: 99,
                    ..Default::default()
                }),
            ),
            (
                GAMEPLAY,
                Body::StorageItemRemoved(net::StorageItemRemoved {
                    kind: 99,
                    ..Default::default()
                }),
            ),
        ]);
        assert!(app.world().resource::<Messages<StorageOpened>>().is_empty());
        assert!(
            app.world()
                .resource::<Messages<StorageItemAdded>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<StorageItemRemoved>>()
                .is_empty()
        );
    }

    #[test]
    fn guild_storage_rejections_are_not_unknown() {
        use net::StorageResultCode::*;
        use net_contract::events::StorageRejection;
        let codes = [
            (StorageNoGuild, StorageRejection::NoGuild),
            (StorageGuildNoSkill, StorageRejection::GuildNoSkill),
            (
                StorageGuildNoPermission,
                StorageRejection::GuildNoPermission,
            ),
            (StorageGuildInUse, StorageRejection::GuildInUse),
            (StorageOtherStorageOpen, StorageRejection::OtherStorageOpen),
            (StorageRental, StorageRejection::Rental),
            (StorageNoGuildStorage, StorageRejection::NoGuildStorage),
            (StorageStale, StorageRejection::Stale),
        ];
        for (code, expected) in codes {
            let app = drain(vec![(
                GAMEPLAY,
                Body::StorageResult(net::StorageResult {
                    result: code as i32,
                }),
            )]);
            let messages = app.world().resource::<Messages<StorageResult>>();
            let results: Vec<_> = messages.iter_current_update_messages().collect();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].outcome, Err(expected), "{code:?}");
        }
    }

    #[test]
    fn unrelated_body_produces_no_storage_messages() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::Announcement(net::Announcement::default()),
        )]);

        assert!(
            app.world()
                .resource::<Messages<StorageOpened>>()
                .iter_current_update_messages()
                .next()
                .is_none()
        );
        assert!(
            app.world()
                .resource::<Messages<StorageItemAdded>>()
                .iter_current_update_messages()
                .next()
                .is_none()
        );
        assert!(
            app.world()
                .resource::<Messages<StorageItemRemoved>>()
                .iter_current_update_messages()
                .next()
                .is_none()
        );
        assert!(
            app.world()
                .resource::<Messages<StorageResult>>()
                .iter_current_update_messages()
                .next()
                .is_none()
        );
    }
}
