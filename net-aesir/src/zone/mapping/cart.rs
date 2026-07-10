use crate::proto::aesir::net;
use net_contract::dto::CartItem;
use net_contract::events::{
    CartItemAdded, CartItemRemoved, CartLoaded, CartMountRejection, CartMountResult,
};

pub fn cart_info(i: net::CartInfo) -> CartLoaded {
    CartLoaded {
        items: i.items.into_iter().map(cart_item).collect(),
        current_weight: i.current_weight,
        max_weight: i.max_weight,
    }
}

fn cart_item(i: net::InventoryItem) -> CartItem {
    CartItem {
        nameid: i.nameid,
        index: i.index,
        amount: i.amount,
        identified: i.identified,
        refine: i.refine,
        cards: i.cards,
        attribute: i.attribute,
        expire_time: i.expire_time,
        weight: i.weight,
    }
}

pub fn cart_item_added(a: net::CartItemAdded) -> CartItemAdded {
    CartItemAdded {
        item: CartItem {
            nameid: a.nameid,
            index: a.index,
            amount: a.amount,
            identified: a.identified,
            refine: a.refine,
            cards: a.cards,
            attribute: a.attribute,
            expire_time: a.expire_time,
            weight: a.weight,
        },
    }
}

pub fn cart_item_removed(r: net::CartItemRemoved) -> CartItemRemoved {
    CartItemRemoved {
        index: r.index as u16,
        amount: r.amount as u16,
        reason: r.reason,
    }
}

/// Maps the server's mount outcome to the neutral event. The two known
/// rejections become `Err`; `CART_OK` and any unrecognised code map to `Ok`,
/// since the only client-actionable outcomes are the rejections.
pub fn cart_mount_result(r: net::CartMountResult) -> CartMountResult {
    let outcome = match net::CartMountResultCode::try_from(r.result) {
        Ok(net::CartMountResultCode::CartSkillNotLearned) => {
            Err(CartMountRejection::SkillNotLearned)
        }
        Ok(net::CartMountResultCode::CartAlreadyMounted) => Err(CartMountRejection::AlreadyMounted),
        _ => Ok(()),
    };
    CartMountResult { outcome }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_cart_item(index: u32, nameid: u32) -> net::InventoryItem {
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
            weight: 15,
        }
    }

    #[test]
    fn cart_info_maps_items_and_weights() {
        let loaded = cart_info(net::CartInfo {
            items: vec![sample_cart_item(0, 501), sample_cart_item(1, 502)],
            current_weight: 30,
            max_weight: 8000,
        });

        assert_eq!(loaded.items.len(), 2);
        assert_eq!(loaded.items[0].nameid, 501);
        assert_eq!(loaded.items[0].weight, 15);
        assert_eq!(loaded.items[1].nameid, 502);
        assert_eq!(loaded.current_weight, 30);
        assert_eq!(loaded.max_weight, 8000);
    }

    #[test]
    fn cart_item_added_maps_weight_and_stack() {
        let added = cart_item_added(net::CartItemAdded {
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
            weight: 20,
        });

        assert_eq!(added.item.index, 3);
        assert_eq!(added.item.amount, 5);
        assert_eq!(added.item.nameid, 501);
        assert_eq!(added.item.cards, vec![10]);
        assert_eq!(added.item.weight, 20);
    }

    #[test]
    fn cart_item_removed_narrows_index_and_amount() {
        let removed = cart_item_removed(net::CartItemRemoved {
            index: 3,
            amount: 2,
            reason: 1,
        });

        assert_eq!(removed.index, 3u16);
        assert_eq!(removed.amount, 2u16);
        assert_eq!(removed.reason, 1);
    }

    #[test]
    fn cart_mount_result_maps_ok_and_rejections() {
        let ok = cart_mount_result(net::CartMountResult {
            result: net::CartMountResultCode::CartOk as i32,
        });
        assert_eq!(ok.outcome, Ok(()));

        let no_skill = cart_mount_result(net::CartMountResult {
            result: net::CartMountResultCode::CartSkillNotLearned as i32,
        });
        assert_eq!(no_skill.outcome, Err(CartMountRejection::SkillNotLearned));

        let already = cart_mount_result(net::CartMountResult {
            result: net::CartMountResultCode::CartAlreadyMounted as i32,
        });
        assert_eq!(already.outcome, Err(CartMountRejection::AlreadyMounted));
    }

    #[test]
    fn cart_mount_result_maps_unknown_code_to_ok() {
        let unknown = cart_mount_result(net::CartMountResult { result: 999 });
        assert_eq!(unknown.outcome, Ok(()));
    }
}
