use crate::infrastructure::networking::quic::proto::aesir::net;
use crate::infrastructure::networking::zone_messages::{
    InventoryReceived, ItemAdded, ItemEquipped, ItemRemoved, ItemUnequipped, ZoneInventoryItem,
};

pub fn inventory_list(l: net::InventoryList) -> InventoryReceived {
    InventoryReceived {
        items: l
            .normal
            .into_iter()
            .chain(l.equip)
            .map(inventory_item)
            .collect(),
    }
}

fn inventory_item(i: net::InventoryItem) -> ZoneInventoryItem {
    ZoneInventoryItem {
        index: i.index,
        nameid: i.nameid,
        type_: i.r#type,
        amount: i.amount,
        location: i.location,
        identified: i.identified,
        attribute: i.attribute,
        refine: i.refine,
        cards: i.cards,
        expire_time: i.expire_time,
        bind_on_equip: i.bind_on_equip,
        favorite: i.favorite,
        look: i.look,
    }
}

pub fn item_added(a: net::ItemAdded) -> ItemAdded {
    ItemAdded {
        index: a.index,
        amount: a.amount,
        nameid: a.nameid,
        identified: a.identified,
        attribute: a.attribute,
        refine: a.refine,
        cards: a.cards,
        location: a.location,
        type_: a.r#type,
        result: a.result,
        expire_time: a.expire_time,
        look: a.look,
    }
}

pub fn item_removed(r: net::ItemRemoved) -> ItemRemoved {
    ItemRemoved {
        index: r.index,
        amount: r.amount,
        reason: r.reason,
    }
}

pub fn equip_result(e: net::EquipResult) -> ItemEquipped {
    ItemEquipped {
        index: e.index,
        wear_location: e.wear_location,
        view_id: e.view_id,
        result: e.result,
    }
}

pub fn unequip_result(u: net::UnequipResult) -> ItemUnequipped {
    ItemUnequipped {
        index: u.index,
        wear_location: u.wear_location,
        result: u.result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item(index: u32, nameid: u32) -> net::InventoryItem {
        net::InventoryItem {
            index,
            nameid,
            r#type: 3,
            amount: 1,
            location: 0,
            identified: true,
            attribute: 0,
            refine: 7,
            cards: vec![100, 200],
            expire_time: 0,
            bind_on_equip: 0,
            favorite: false,
            look: 0,
        }
    }

    #[test]
    fn inventory_list_flattens_normal_and_equip() {
        let received = inventory_list(net::InventoryList {
            normal: vec![sample_item(0, 501), sample_item(1, 502)],
            equip: vec![sample_item(2, 1201)],
        });

        assert_eq!(received.items.len(), 3);
        assert_eq!(received.items[0].nameid, 501);
        assert_eq!(received.items[1].nameid, 502);
        assert_eq!(received.items[2].nameid, 1201);
    }

    #[test]
    fn inventory_item_preserves_cards_and_refine() {
        let received = inventory_list(net::InventoryList {
            normal: vec![sample_item(5, 1201)],
            equip: vec![],
        });

        let item = &received.items[0];
        assert_eq!(item.index, 5);
        assert_eq!(item.type_, 3);
        assert_eq!(item.refine, 7);
        assert_eq!(item.cards, vec![100, 200]);
        assert!(item.identified);
    }

    #[test]
    fn item_added_maps_type_and_result() {
        let added = item_added(net::ItemAdded {
            index: 3,
            amount: 5,
            nameid: 501,
            identified: true,
            attribute: 0,
            refine: 0,
            cards: vec![10],
            location: 0,
            r#type: 2,
            result: 0,
            expire_time: 0,
            look: 0,
        });

        assert_eq!(added.index, 3);
        assert_eq!(added.amount, 5);
        assert_eq!(added.nameid, 501);
        assert_eq!(added.type_, 2);
        assert_eq!(added.result, 0);
        assert_eq!(added.cards, vec![10]);
    }

    #[test]
    fn item_removed_maps_reason() {
        let removed = item_removed(net::ItemRemoved {
            index: 3,
            amount: 2,
            reason: 1,
        });

        assert_eq!(removed.index, 3);
        assert_eq!(removed.amount, 2);
        assert_eq!(removed.reason, 1);
    }

    #[test]
    fn equip_result_maps_location_and_view() {
        let equipped = equip_result(net::EquipResult {
            index: 3,
            wear_location: 16,
            view_id: 5,
            result: 1,
        });

        assert_eq!(equipped.index, 3);
        assert_eq!(equipped.wear_location, 16);
        assert_eq!(equipped.view_id, 5);
        assert_eq!(equipped.result, 1);
    }

    #[test]
    fn unequip_result_maps_location() {
        let unequipped = unequip_result(net::UnequipResult {
            index: 3,
            wear_location: 16,
            result: 1,
        });

        assert_eq!(unequipped.index, 3);
        assert_eq!(unequipped.wear_location, 16);
        assert_eq!(unequipped.result, 1);
    }
}
