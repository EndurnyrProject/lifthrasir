use super::resource::Storage;
use bevy::prelude::*;
use net_contract::commands::CloseStorage;
use net_contract::events::{StorageItemAdded, StorageItemRemoved, StorageOpened};

pub fn apply_storage_opened(
    mut opened: MessageReader<StorageOpened>,
    mut storage: ResMut<Storage>,
) {
    for snapshot in opened.read() {
        storage.open(snapshot.capacity, snapshot.items.clone());
    }
}

pub fn apply_storage_item_deltas(
    mut added: MessageReader<StorageItemAdded>,
    mut removed: MessageReader<StorageItemRemoved>,
    mut storage: ResMut<Storage>,
) {
    if !storage.is_open() {
        let ignored = added.read().count() + removed.read().count();
        if ignored > 0 {
            warn!(
                ignored,
                "ignoring Storage item deltas while Storage is closed"
            );
        }
        return;
    }

    for event in added.read() {
        storage.upsert(event.item.clone());
    }
    for event in removed.read() {
        storage.remove_amount(event.index, event.amount);
    }
}

pub fn apply_storage_close(mut closed: MessageReader<CloseStorage>, mut storage: ResMut<Storage>) {
    if closed.read().next().is_some() {
        storage.close();
    }
}

pub fn reset_storage(mut storage: ResMut<Storage>) {
    storage.reset();
}

#[cfg(test)]
mod tests {
    use crate::core::state::GameState;
    use crate::domain::storage::{Storage, StoragePlugin};
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;
    use net_contract::commands::CloseStorage;
    use net_contract::dto::StorageItem;
    use net_contract::events::{StorageItemAdded, StorageItemRemoved, StorageOpened};

    fn item(index: u32, amount: u32) -> StorageItem {
        StorageItem {
            index,
            nameid: 501,
            amount,
            type_: 0,
            location: 0,
            attribute: 0,
            refine: 0,
            expire_time: 0,
            look: 0,
            weight: 10,
            identified: true,
            cards: vec![],
        }
    }

    fn app_with_storage() -> App {
        let mut app = App::new();
        app.add_message::<StorageOpened>();
        app.add_message::<StorageItemAdded>();
        app.add_message::<StorageItemRemoved>();
        app.add_message::<CloseStorage>();
        app.add_plugins(StoragePlugin);
        app
    }

    #[test]
    fn snapshot_is_applied_before_same_frame_deltas() {
        let mut app = app_with_storage();
        app.world_mut().write_message(StorageOpened {
            capacity: 40,
            items: vec![item(7, 10)],
        });
        app.world_mut()
            .write_message(StorageItemAdded { item: item(7, 7) });
        app.world_mut().write_message(StorageItemRemoved {
            index: 7,
            amount: 2,
            reason: 0,
        });

        app.update();

        let storage = app.world().resource::<Storage>();
        assert!(storage.is_open());
        assert_eq!(storage.capacity(), 40);
        assert_eq!(storage.get(7).unwrap().amount, 5);
    }

    #[test]
    fn deltas_while_closed_are_ignored() {
        let mut app = app_with_storage();
        app.world_mut()
            .write_message(StorageItemAdded { item: item(7, 7) });
        app.world_mut().write_message(StorageItemRemoved {
            index: 7,
            amount: 2,
            reason: 0,
        });

        app.update();

        let storage = app.world().resource::<Storage>();
        assert!(!storage.is_open());
        assert!(storage.is_empty());
    }

    #[derive(Resource, Default)]
    struct CloseMessageCount(usize);

    fn count_close_messages(
        mut messages: MessageReader<CloseStorage>,
        mut count: ResMut<CloseMessageCount>,
    ) {
        count.0 += messages.read().count();
    }

    #[test]
    fn close_is_applied_after_deltas_and_remains_available_to_other_readers() {
        let mut app = app_with_storage();
        app.init_resource::<CloseMessageCount>();
        app.add_systems(Update, count_close_messages);
        app.world_mut().write_message(StorageOpened {
            capacity: 40,
            items: vec![item(7, 10)],
        });
        app.world_mut()
            .write_message(StorageItemAdded { item: item(7, 7) });
        app.world_mut().write_message(CloseStorage);

        app.update();

        let storage = app.world().resource::<Storage>();
        assert!(!storage.is_open());
        assert_eq!(storage.get(7).unwrap().amount, 7);
        assert_eq!(app.world().resource::<CloseMessageCount>().0, 1);
    }

    #[test]
    fn leaving_in_game_resets_storage() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.add_message::<StorageOpened>();
        app.add_message::<StorageItemAdded>();
        app.add_message::<StorageItemRemoved>();
        app.add_message::<CloseStorage>();
        app.add_plugins(StoragePlugin);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        app.world_mut().write_message(StorageOpened {
            capacity: 40,
            items: vec![item(7, 10)],
        });
        app.update();
        assert!(app.world().resource::<Storage>().is_open());

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Login);
        app.update();

        let storage = app.world().resource::<Storage>();
        assert!(!storage.is_open());
        assert_eq!(storage.capacity(), 0);
        assert!(storage.is_empty());
    }
}
