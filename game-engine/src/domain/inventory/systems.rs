use super::events::{InventoryDumpCompleted, InventoryDumpStarted, InventoryItemsReceived};
use super::resource::Inventory;
use bevy::prelude::*;

pub fn apply_inventory_messages(
    mut started: MessageReader<InventoryDumpStarted>,
    mut received: MessageReader<InventoryItemsReceived>,
    mut completed: MessageReader<InventoryDumpCompleted>,
    mut inventory: ResMut<Inventory>,
) {
    if started.is_empty() && received.is_empty() && completed.is_empty() {
        return;
    }

    for _ in started.read() {
        inventory.begin();
    }
    for batch in received.read() {
        for item in &batch.items {
            inventory.upsert(item.clone());
        }
    }
    for _ in completed.read() {
        inventory.finish();
    }
}

pub fn reset_inventory(mut inventory: ResMut<Inventory>) {
    *inventory = Inventory::default();
}

#[cfg(test)]
mod tests {
    use crate::core::state::GameState;
    use crate::domain::inventory::{
        Inventory, InventoryDumpCompleted, InventoryDumpStarted, InventoryItemsReceived,
        InventoryPlugin, Item,
    };
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;

    fn equip(index: u16) -> Item {
        Item {
            index,
            wear_state: 1,
            amount: 1,
            ..Default::default()
        }
    }

    fn stackable(index: u16, amount: u16) -> Item {
        Item {
            index,
            wear_state: 0,
            amount,
            ..Default::default()
        }
    }

    fn write<M: Message>(app: &mut App, message: M) {
        app.world_mut().write_message(message);
    }

    #[test]
    fn dump_populates_inventory_resource() {
        let mut app = App::new();
        app.add_plugins(InventoryPlugin);

        write(&mut app, InventoryDumpStarted);
        write(
            &mut app,
            InventoryItemsReceived {
                items: vec![stackable(2, 5), stackable(3, 9), equip(4)],
            },
        );
        write(&mut app, InventoryDumpCompleted);

        app.update();

        let inventory = app.world().resource::<Inventory>();
        assert_eq!(inventory.len(), 3);
        assert_eq!(inventory.equipped().count(), 1);
        assert!(inventory.is_ready());
    }

    #[test]
    fn new_dump_clears_prior_items() {
        let mut app = App::new();
        app.add_plugins(InventoryPlugin);

        write(&mut app, InventoryDumpStarted);
        write(
            &mut app,
            InventoryItemsReceived {
                items: vec![stackable(2, 5), stackable(3, 9)],
            },
        );
        write(&mut app, InventoryDumpCompleted);
        app.update();
        assert_eq!(app.world().resource::<Inventory>().len(), 2);

        write(&mut app, InventoryDumpStarted);
        write(
            &mut app,
            InventoryItemsReceived {
                items: vec![equip(4)],
            },
        );
        write(&mut app, InventoryDumpCompleted);
        app.update();

        let inventory = app.world().resource::<Inventory>();
        assert_eq!(inventory.len(), 1);
        assert_eq!(inventory.equipped().count(), 1);
        assert!(inventory.is_ready());
    }

    #[test]
    fn leaving_ingame_empties_inventory() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.add_plugins(InventoryPlugin);

        write(&mut app, InventoryDumpStarted);
        write(
            &mut app,
            InventoryItemsReceived {
                items: vec![stackable(2, 5)],
            },
        );
        write(&mut app, InventoryDumpCompleted);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        assert_eq!(app.world().resource::<Inventory>().len(), 1);
        assert!(app.world().resource::<Inventory>().is_ready());

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Loading);
        app.update();

        let inventory = app.world().resource::<Inventory>();
        assert_eq!(inventory.len(), 0);
        assert!(!inventory.is_ready());
    }

    #[test]
    fn no_messages_leaves_inventory_unchanged() {
        let mut app = App::new();
        app.add_plugins(InventoryPlugin);

        app.update();
        let before = app
            .world()
            .get_resource_change_ticks::<Inventory>()
            .expect("inventory resource exists")
            .changed;

        app.update();

        let after = app
            .world()
            .get_resource_change_ticks::<Inventory>()
            .expect("inventory resource exists")
            .changed;
        assert_eq!(before, after);
    }
}
