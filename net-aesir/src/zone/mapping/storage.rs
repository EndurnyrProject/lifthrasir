use crate::proto::aesir::net;
use bevy::prelude::warn;
use net_contract::dto::StorageItem;
use net_contract::events::{
    StorageItemAdded, StorageItemRemoved, StorageOpened, StorageRejection, StorageResult,
};

pub fn storage_opened(opened: net::StorageOpened) -> StorageOpened {
    StorageOpened {
        capacity: opened.capacity,
        items: opened.items.into_iter().map(storage_item).collect(),
    }
}

fn storage_item(item: net::InventoryItem) -> StorageItem {
    StorageItem {
        index: item.index,
        nameid: item.nameid,
        amount: item.amount,
        type_: item.r#type,
        location: item.location,
        attribute: item.attribute,
        refine: item.refine,
        expire_time: item.expire_time,
        look: item.look,
        weight: item.weight,
        identified: item.identified,
        cards: item.cards,
    }
}

pub fn storage_item_added(added: net::StorageItemAdded) -> StorageItemAdded {
    StorageItemAdded {
        item: StorageItem {
            index: added.index,
            nameid: added.nameid,
            amount: added.amount,
            type_: added.r#type,
            location: added.location,
            attribute: added.attribute,
            refine: added.refine,
            expire_time: added.expire_time,
            look: added.look,
            weight: added.weight,
            identified: added.identified,
            cards: added.cards,
        },
    }
}

pub fn storage_item_removed(removed: net::StorageItemRemoved) -> StorageItemRemoved {
    StorageItemRemoved {
        index: removed.index,
        amount: removed.amount,
        reason: removed.reason,
    }
}

pub fn storage_result(result: net::StorageResult) -> StorageResult {
    use net::StorageResultCode::*;

    let outcome = match net::StorageResultCode::try_from(result.result) {
        Ok(StorageOk) => Ok(()),
        Ok(StorageFull) => Err(StorageRejection::Full),
        Ok(StorageInventoryFull) => Err(StorageRejection::InventoryFull),
        Ok(StorageOverweight) => Err(StorageRejection::Overweight),
        Ok(StorageNotStorable) => Err(StorageRejection::NotStorable),
        Ok(StorageItemEquipped) => Err(StorageRejection::ItemEquipped),
        Ok(StorageInvalidAmount) => Err(StorageRejection::InvalidAmount),
        Ok(StorageNotOpen) => Err(StorageRejection::NotOpen),
        Ok(StorageBasicSkillRequired) => Err(StorageRejection::BasicSkillRequired),
        Err(_) => {
            warn!("unknown Storage result code {}", result.result);
            Err(StorageRejection::Unknown(result.result))
        }
    };
    StorageResult { outcome }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_opened_maps_capacity_and_every_item_field_losslessly() {
        let opened = storage_opened(net::StorageOpened {
            capacity: 600,
            items: vec![net::InventoryItem {
                index: 70_000,
                nameid: 501,
                r#type: 2,
                amount: 80_000,
                location: 4,
                identified: true,
                attribute: 3,
                refine: 7,
                cards: vec![4001, 4002],
                expire_time: u32::MAX as u64 + 1,
                bind_on_equip: 1,
                favorite: true,
                look: 9,
                weight: 15,
            }],
        });

        assert_eq!(opened.capacity, 600);
        assert_eq!(opened.items.len(), 1);
        let item = &opened.items[0];
        assert_eq!(item.index, 70_000);
        assert_eq!(item.nameid, 501);
        assert_eq!(item.amount, 80_000);
        assert_eq!(item.type_, 2);
        assert_eq!(item.location, 4);
        assert_eq!(item.attribute, 3);
        assert_eq!(item.refine, 7);
        assert_eq!(item.expire_time, u32::MAX as u64 + 1);
        assert_eq!(item.look, 9);
        assert_eq!(item.weight, 15);
        assert!(item.identified);
        assert_eq!(item.cards, vec![4001, 4002]);
    }

    #[test]
    fn storage_item_added_maps_every_item_field_losslessly() {
        let added = storage_item_added(net::StorageItemAdded {
            index: 70_001,
            amount: 80_001,
            nameid: 502,
            identified: true,
            attribute: 4,
            refine: 8,
            cards: vec![4003, 4004],
            location: 16,
            r#type: 5,
            result: 12,
            expire_time: u32::MAX as u64 + 2,
            look: 10,
            weight: 20,
        });

        let item = added.item;
        assert_eq!(item.index, 70_001);
        assert_eq!(item.nameid, 502);
        assert_eq!(item.amount, 80_001);
        assert_eq!(item.type_, 5);
        assert_eq!(item.location, 16);
        assert_eq!(item.attribute, 4);
        assert_eq!(item.refine, 8);
        assert_eq!(item.expire_time, u32::MAX as u64 + 2);
        assert_eq!(item.look, 10);
        assert_eq!(item.weight, 20);
        assert!(item.identified);
        assert_eq!(item.cards, vec![4003, 4004]);
    }

    #[test]
    fn storage_item_removed_preserves_u32_index_amount_and_reason() {
        let removed = storage_item_removed(net::StorageItemRemoved {
            index: 70_002,
            amount: 80_002,
            reason: 3,
        });

        assert_eq!(removed.index, 70_002);
        assert_eq!(removed.amount, 80_002);
        assert_eq!(removed.reason, 3);
    }

    #[test]
    fn storage_result_maps_every_known_code() {
        use net::StorageResultCode::*;

        let cases = [
            (StorageOk, Ok(())),
            (StorageFull, Err(StorageRejection::Full)),
            (StorageInventoryFull, Err(StorageRejection::InventoryFull)),
            (StorageOverweight, Err(StorageRejection::Overweight)),
            (StorageNotStorable, Err(StorageRejection::NotStorable)),
            (StorageItemEquipped, Err(StorageRejection::ItemEquipped)),
            (StorageInvalidAmount, Err(StorageRejection::InvalidAmount)),
            (StorageNotOpen, Err(StorageRejection::NotOpen)),
            (
                StorageBasicSkillRequired,
                Err(StorageRejection::BasicSkillRequired),
            ),
        ];

        for (code, expected) in cases {
            let result = storage_result(net::StorageResult {
                result: code as i32,
            });
            assert_eq!(result.outcome, expected, "code {code:?}");
        }
    }

    #[test]
    fn storage_result_preserves_unknown_code_as_rejection() {
        let result = storage_result(net::StorageResult { result: 999 });

        assert_eq!(result.outcome, Err(StorageRejection::Unknown(999)));
    }
}
