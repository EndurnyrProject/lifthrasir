use super::item::Item;
use super::resource::Inventory;
use crate::infrastructure::networking::zone_messages::{
    InventoryReceived, ItemAdded, ItemRemoved, ZoneInventoryItem,
};
use bevy::prelude::*;

/// Maps a proto-shaped inventory slot onto the domain `Item`.
///
/// aesir sends the *worn* bitmask in `location` (`item.equip` server-side; 0 for a
/// bag item), so `wear_state` is taken straight from it — that is what drives
/// `is_equipped()`, the paperdoll, and the inventory filter on (re)login.
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
        wear_state: slot.location,
        refine: slot.refine as u8,
        cards,
        options: vec![],
        expire_time: slot.expire_time as u32,
        view_sprite: slot.look as u16,
        identified: slot.identified,
        damaged: slot.attribute != 0,
    }
}

fn item_from_added(a: &ItemAdded) -> Item {
    Item {
        index: a.index as u16,
        item_id: a.nameid,
        item_type: a.type_ as u8,
        amount: a.amount as u16,
        location: a.location,
        wear_state: a.location,
        refine: a.refine as u8,
        cards: std::array::from_fn(|i| a.cards.get(i).copied().unwrap_or(0)),
        options: vec![],
        expire_time: a.expire_time as u32,
        view_sprite: a.look as u16,
        identified: a.identified,
        damaged: a.attribute != 0,
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

pub fn apply_item_deltas(
    mut added: MessageReader<ItemAdded>,
    mut removed: MessageReader<ItemRemoved>,
    mut inventory: ResMut<Inventory>,
) {
    for a in added.read() {
        inventory.upsert(item_from_added(a));
    }
    for r in removed.read() {
        inventory.remove_amount(r.index as u16, r.amount as u16);
    }
}

pub fn reset_inventory(mut inventory: ResMut<Inventory>) {
    *inventory = Inventory::default();
}

#[cfg(test)]
mod tests {
    use super::item_from_added;
    use crate::core::state::GameState;
    use crate::domain::inventory::{Inventory, InventoryPlugin};
    use crate::infrastructure::networking::zone_messages::{
        InventoryReceived, ItemAdded, ItemRemoved, ZoneInventoryItem,
    };
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
        app.add_message::<ItemAdded>();
        app.add_message::<ItemRemoved>();
        app.add_plugins(InventoryPlugin);
        app
    }

    fn added(index: u32, amount: u32) -> ItemAdded {
        ItemAdded {
            index,
            amount,
            nameid: 0,
            identified: true,
            attribute: 0,
            refine: 0,
            cards: vec![],
            location: 0,
            type_: 0,
            result: 0,
            expire_time: 0,
            look: 0,
        }
    }

    fn removed(index: u32, amount: u32) -> ItemRemoved {
        ItemRemoved {
            index,
            amount,
            reason: 0,
        }
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
        app.add_message::<ItemAdded>();
        app.add_message::<ItemRemoved>();
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

    #[test]
    fn item_from_added_maps_fields() {
        let mut a = added(7, 4);
        a.nameid = 501;
        a.type_ = 5;
        a.refine = 3;
        a.cards = vec![10, 20, 30, 40];
        a.identified = false;
        a.attribute = 1;

        let item = item_from_added(&a);

        assert_eq!(item.index, 7);
        assert_eq!(item.item_id, 501);
        assert_eq!(item.item_type, 5);
        assert_eq!(item.amount, 4);
        assert_eq!(item.refine, 3);
        assert_eq!(item.cards, [10, 20, 30, 40]);
        assert!(!item.identified);
        assert!(item.damaged);
    }

    #[test]
    fn item_added_replaces_slot_total() {
        let mut app = app_with_inventory();

        app.world_mut().write_message(dump(vec![slot(7, 5)]));
        app.update();
        assert_eq!(
            app.world().resource::<Inventory>().get(7).unwrap().amount,
            5
        );

        app.world_mut().write_message(added(7, 8));
        app.update();

        let inventory = app.world().resource::<Inventory>();
        assert_eq!(inventory.get(7).unwrap().amount, 8);
        assert_eq!(inventory.len(), 1);
    }

    #[test]
    fn item_removed_decrements_stack() {
        let mut app = app_with_inventory();

        app.world_mut().write_message(dump(vec![slot(7, 5)]));
        app.update();

        app.world_mut().write_message(removed(7, 2));
        app.update();

        assert_eq!(
            app.world().resource::<Inventory>().get(7).unwrap().amount,
            3
        );
    }

    #[test]
    fn item_removed_drops_last_unit() {
        let mut app = app_with_inventory();

        app.world_mut().write_message(dump(vec![slot(7, 1)]));
        app.update();

        app.world_mut().write_message(removed(7, 1));
        app.update();

        let inventory = app.world().resource::<Inventory>();
        assert!(inventory.get(7).is_none());
        assert_eq!(inventory.len(), 0);
    }
}
