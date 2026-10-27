//! One outstanding deal outlives its single-use shop window.

use bevy::prelude::*;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use net_contract::{
    dto::ShopResult,
    events::{ChatHeard, ShopBuyResulted, ShopSellResulted},
};

use super::trade_summary;

#[derive(Resource, Default)]
pub(super) struct PendingShopDeal(pub Option<ShopDeal>);

pub(super) struct ShopDeal {
    pub unit_id: u64,
    pub buy: bool,
    pub lines: Vec<(u32, u32)>,
}

/// Results settle a submitted deal, never a subsequently opened shop draft.
pub(super) fn on_shop_result(
    mut buy_results: MessageReader<ShopBuyResulted>,
    mut sell_results: MessageReader<ShopSellResulted>,
    mut pending: ResMut<PendingShopDeal>,
    item_db: Option<Res<ItemDb>>,
    mut chat: MessageWriter<ChatHeard>,
) {
    for event in buy_results.read() {
        if let Some(message) = settle(&mut pending, true, event.result, item_db.as_deref()) {
            chat.write(ChatHeard { gid: 0, message });
        }
    }
    for event in sell_results.read() {
        if let Some(message) = settle(&mut pending, false, event.result, item_db.as_deref()) {
            chat.write(ChatHeard { gid: 0, message });
        }
    }
}

fn settle(
    pending: &mut PendingShopDeal,
    buy: bool,
    result: ShopResult,
    item_db: Option<&ItemDb>,
) -> Option<String> {
    if pending.0.as_ref()?.buy != buy {
        return None;
    }
    let deal = pending.0.take()?;
    debug!("settled NPC shop {} deal: {result:?}", deal.unit_id);
    if result == ShopResult::Ok {
        return Some(trade_summary(buy, &deal.lines, item_db));
    }
    let reason = match result {
        ShopResult::NotEnoughZeny => "Not enough zeny.",
        ShopResult::Overweight => "You are carrying too much weight.",
        ShopResult::NoSlots => "Your inventory has no free slots.",
        ShopResult::Invalid => "The shop transaction was invalid.",
        ShopResult::OutOfRange => "You are too far from the shop.",
        ShopResult::Ok => return None,
    };
    Some(format!(
        "^FF0000{reason} Reopen the shop to try again.^000000"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_without_a_window_settle_once_and_report_success_or_rejection() {
        for (buy, result) in [(true, ShopResult::NotEnoughZeny), (false, ShopResult::Ok)] {
            let mut app = App::new();
            app.insert_resource(PendingShopDeal(Some(ShopDeal {
                unit_id: 7,
                buy,
                lines: vec![(501, 5)],
            })));
            app.add_message::<ShopBuyResulted>()
                .add_message::<ShopSellResulted>()
                .add_message::<ChatHeard>()
                .add_systems(Update, on_shop_result);
            if buy {
                app.world_mut().write_message(ShopBuyResulted { result });
                app.world_mut().write_message(ShopBuyResulted { result });
            } else {
                app.world_mut().write_message(ShopSellResulted { result });
                app.world_mut().write_message(ShopSellResulted { result });
            }
            app.update();
            assert!(app.world().resource::<PendingShopDeal>().0.is_none());
            let lines: Vec<_> = app
                .world()
                .resource::<Messages<ChatHeard>>()
                .iter_current_update_messages()
                .collect();
            assert_eq!(lines.len(), 1);
            if buy {
                assert!(lines[0].message.contains("Reopen"));
            } else {
                assert_eq!(lines[0].message, "Sold 5x #501");
            }
        }
    }
}
