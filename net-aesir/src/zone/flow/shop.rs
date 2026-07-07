use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::shop::{buy_result, sell_result, shop_opened};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{ShopBuyResulted, ShopOpened, ShopSellResulted};

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_shop(
    mut incoming: MessageReader<IncomingMessage>,
    mut opened: MessageWriter<ShopOpened>,
    mut buy: MessageWriter<ShopBuyResulted>,
    mut sell: MessageWriter<ShopSellResulted>,
) {
    for msg in incoming.read() {
        match msg.body.clone() {
            Body::NpcShopOpen(o) => {
                opened.write(shop_opened(o));
            }
            Body::NpcBuyResult(r) => {
                buy.write(buy_result(r));
            }
            Body::NpcSellResult(r) => {
                sell.write(sell_result(r));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::WORLD;
    use crate::proto::aesir::net;

    fn drain(bodies: Vec<(u8, Body)>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<ShopOpened>()
            .add_message::<ShopBuyResulted>()
            .add_message::<ShopSellResulted>()
            .add_systems(Update, zone_drain_shop);

        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for (channel, body) in bodies {
            incoming.write(IncomingMessage { channel, body });
        }
        app.update();
        app
    }

    #[test]
    fn npc_shop_open_produces_one_shop_opened() {
        let app = drain(vec![(
            WORLD,
            Body::NpcShopOpen(net::NpcShopOpen {
                unit_id: 150001,
                buy_items: vec![],
                sell_items: vec![],
            }),
        )]);

        let opened = app.world().resource::<Messages<ShopOpened>>();
        let events: Vec<_> = opened.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].unit_id, 150001);
    }

    #[test]
    fn npc_buy_result_produces_one_shop_buy_resulted() {
        let app = drain(vec![(
            WORLD,
            Body::NpcBuyResult(net::NpcBuyResult { result: 1 }),
        )]);

        let buy = app.world().resource::<Messages<ShopBuyResulted>>();
        let events: Vec<_> = buy.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn npc_sell_result_produces_one_shop_sell_resulted() {
        let app = drain(vec![(
            WORLD,
            Body::NpcSellResult(net::NpcSellResult { result: 0 }),
        )]);

        let sell = app.world().resource::<Messages<ShopSellResulted>>();
        let events: Vec<_> = sell.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn unrelated_body_produces_no_shop_events() {
        let app = drain(vec![(
            WORLD,
            Body::ChatMessage(net::ChatMessage {
                gid: 150001,
                message: "hello".into(),
            }),
        )]);

        let opened = app.world().resource::<Messages<ShopOpened>>();
        let buy = app.world().resource::<Messages<ShopBuyResulted>>();
        let sell = app.world().resource::<Messages<ShopSellResulted>>();
        assert_eq!(opened.iter_current_update_messages().count(), 0);
        assert_eq!(buy.iter_current_update_messages().count(), 0);
        assert_eq!(sell.iter_current_update_messages().count(), 0);
    }
}
