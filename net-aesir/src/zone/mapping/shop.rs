use crate::proto::aesir::net;
use bevy::prelude::warn;
use net_contract::dto::{ShopBuyItem, ShopResult, ShopSellItem};
use net_contract::events::{ShopBuyResulted, ShopOpened, ShopSellResulted};

pub fn shop_opened(o: net::NpcShopOpen) -> ShopOpened {
    ShopOpened {
        unit_id: o.unit_id,
        buy_items: o
            .buy_items
            .into_iter()
            .map(|i| ShopBuyItem {
                nameid: i.nameid,
                price: i.price,
            })
            .collect(),
        sell_items: o
            .sell_items
            .into_iter()
            .map(|i| ShopSellItem {
                inventory_index: i.inventory_index,
                nameid: i.nameid,
                amount: i.amount,
                sell_price: i.sell_price,
            })
            .collect(),
    }
}

pub fn buy_result(r: net::NpcBuyResult) -> ShopBuyResulted {
    ShopBuyResulted {
        result: result_from_code(r.result),
    }
}

pub fn sell_result(r: net::NpcSellResult) -> ShopSellResulted {
    ShopSellResulted {
        result: result_from_code(r.result),
    }
}

pub fn result_from_code(code: u32) -> ShopResult {
    match code {
        0 => ShopResult::Ok,
        1 => ShopResult::NotEnoughZeny,
        2 => ShopResult::Overweight,
        3 => ShopResult::NoSlots,
        4 => ShopResult::Invalid,
        5 => ShopResult::OutOfRange,
        _ => {
            warn!("unknown npc shop result code {}; treating as Invalid", code);
            ShopResult::Invalid
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_from_code_maps_all_known_codes() {
        assert_eq!(result_from_code(0), ShopResult::Ok);
        assert_eq!(result_from_code(1), ShopResult::NotEnoughZeny);
        assert_eq!(result_from_code(2), ShopResult::Overweight);
        assert_eq!(result_from_code(3), ShopResult::NoSlots);
        assert_eq!(result_from_code(4), ShopResult::Invalid);
        assert_eq!(result_from_code(5), ShopResult::OutOfRange);
    }

    #[test]
    fn result_from_code_maps_unknown_to_invalid() {
        assert_eq!(result_from_code(99), ShopResult::Invalid);
    }

    #[test]
    fn shop_opened_maps_fields() {
        let opened = shop_opened(net::NpcShopOpen {
            unit_id: 150001,
            buy_items: vec![net::NpcShopBuyItem {
                nameid: 501,
                r#type: 0,
                price: 50,
            }],
            sell_items: vec![net::NpcShopSellItem {
                inventory_index: 3,
                nameid: 502,
                r#type: 0,
                amount: 2,
                sell_price: 25,
            }],
        });

        assert_eq!(opened.unit_id, 150001);
        assert_eq!(opened.buy_items.len(), 1);
        assert_eq!(opened.buy_items[0].nameid, 501);
        assert_eq!(opened.buy_items[0].price, 50);
        assert_eq!(opened.sell_items.len(), 1);
        assert_eq!(opened.sell_items[0].inventory_index, 3);
        assert_eq!(opened.sell_items[0].nameid, 502);
        assert_eq!(opened.sell_items[0].amount, 2);
        assert_eq!(opened.sell_items[0].sell_price, 25);
    }

    #[test]
    fn buy_result_maps_code() {
        let result = buy_result(net::NpcBuyResult { result: 1 });
        assert_eq!(result.result, ShopResult::NotEnoughZeny);
    }

    #[test]
    fn sell_result_maps_code() {
        let result = sell_result(net::NpcSellResult { result: 5 });
        assert_eq!(result.result, ShopResult::OutOfRange);
    }
}
