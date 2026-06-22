use super::item::Item;
use super::resource::Inventory;
use crate::infrastructure::networking::zone_messages::{InventoryReceived, ZoneInventoryItem};
use bevy::prelude::*;

/// Maps a proto-shaped inventory slot onto the domain `Item`.
///
/// NOTE: the proto carries `location` (allowed equip slots) but no worn bitmask;
/// `wear_state` (and thus `is_equipped()`) defaults to 0 until equip results wire it.
fn to_item(slot: &ZoneInventoryItem) -> Item {
    let mut cards = [0u32; 4];
    for (dst, src) in cards.iter_mut().zip(slot.cards.iter()) {
        *dst = *src;
    }

    Item {
        index: slot.index as u16,
        item_id: slot.nameid,
        item_type: slot.type_ as u8,
        amount: slot.amount as u16,
        location: slot.location,
        wear_state: 0,
        refine: slot.refine as u8,
        cards,
        options: vec![],
        expire_time: slot.expire_time as u32,
        view_sprite: slot.look as u16,
        identified: slot.identified,
        damaged: slot.attribute != 0,
    }
}

pub fn apply_inventory_messages(
    mut received: MessageReader<InventoryReceived>,
    mut inventory: ResMut<Inventory>,
) {
    for dump in received.read() {
        inventory.begin();
        for slot in &dump.items {
            inventory.upsert(to_item(slot));
        }
        inventory.finish();
    }
}

pub fn reset_inventory(mut inventory: ResMut<Inventory>) {
    *inventory = Inventory::default();
}

#[cfg(test)]
mod tests {
    use crate::core::state::GameState;
    use crate::domain::inventory::{Inventory, InventoryPlugin};
    use crate::infrastructure::networking::zone_messages::{InventoryReceived, ZoneInventoryItem};
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;

    fn slot(index: u32, amount: u32) -> ZoneInventoryItem {
        ZoneInventoryItem {
            index,
            nameid: 0,
            type_: 0,
            amount,
            location: 0,
            identified: true,
            attribute: 0,
            refine: 0,
            cards: vec![],
            expire_time: 0,
            bind_on_equip: 0,
            favorite: false,
            look: 0,
        }
    }

    fn app_with_inventory() -> App {
        let mut app = App::new();
        app.add_message::<InventoryReceived>();
        app.add_plugins(InventoryPlugin);
        app
    }

    fn dump(items: Vec<ZoneInventoryItem>) -> InventoryReceived {
        InventoryReceived { items }
    }

    #[test]
    fn dump_populates_inventory_resource() {
        let mut app = app_with_inventory();

        app.world_mut()
            .write_message(dump(vec![slot(2, 5), slot(3, 9), slot(4, 1)]));
        app.update();

        let inventory = app.world().resource::<Inventory>();
        assert_eq!(inventory.len(), 3);
        assert!(inventory.is_ready());
    }

    #[test]
    fn new_dump_clears_prior_items() {
        let mut app = app_with_inventory();

        app.world_mut()
            .write_message(dump(vec![slot(2, 5), slot(3, 9)]));
        app.update();
        assert_eq!(app.world().resource::<Inventory>().len(), 2);

        app.world_mut().write_message(dump(vec![slot(4, 1)]));
        app.update();

        let inventory = app.world().resource::<Inventory>();
        assert_eq!(inventory.len(), 1);
        assert!(inventory.is_ready());
    }

    #[test]
    fn entering_character_selection_empties_inventory() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.add_message::<InventoryReceived>();
        app.add_plugins(InventoryPlugin);

        app.world_mut().write_message(dump(vec![slot(2, 5)]));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        assert_eq!(app.world().resource::<Inventory>().len(), 1);
        assert!(app.world().resource::<Inventory>().is_ready());

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::CharacterSelection);
        app.update();

        let inventory = app.world().resource::<Inventory>();
        assert_eq!(inventory.len(), 0);
        assert!(!inventory.is_ready());
    }

    #[test]
    fn no_messages_leaves_inventory_unchanged() {
        let mut app = app_with_inventory();

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
