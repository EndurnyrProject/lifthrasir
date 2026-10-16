use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use net_contract::events::{
    TradeCancelled, TradeCompleted, TradeOfferUpdated, TradeOpened, TradeRequestNotified,
};

use super::super::mapping::trade::{
    trade_cancelled, trade_offer_update, trade_opened, trade_request_received,
};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_trade(
    mut incoming: MessageReader<IncomingMessage>,
    mut requests: MessageWriter<TradeRequestNotified>,
    mut opened: MessageWriter<TradeOpened>,
    mut offers: MessageWriter<TradeOfferUpdated>,
    mut completed: MessageWriter<TradeCompleted>,
    mut cancelled: MessageWriter<TradeCancelled>,
) {
    for message in incoming.read() {
        match message.body.clone() {
            Body::TradeRequestReceived(body) => {
                requests.write(trade_request_received(body));
            }
            Body::TradeOpened(body) => {
                opened.write(trade_opened(body));
            }
            Body::TradeOfferUpdate(body) => {
                offers.write(trade_offer_update(body));
            }
            Body::TradeCompleted(_) => {
                completed.write(TradeCompleted);
            }
            Body::TradeCancelled(body) => {
                cancelled.write(trade_cancelled(body));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{channels::GAMEPLAY, proto::aesir::net};

    fn drain(bodies: Vec<Body>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<TradeRequestNotified>()
            .add_message::<TradeOpened>()
            .add_message::<TradeOfferUpdated>()
            .add_message::<TradeCompleted>()
            .add_message::<TradeCancelled>()
            .add_systems(Update, zone_drain_trade);
        for body in bodies {
            app.world_mut().write_message(IncomingMessage {
                channel: GAMEPLAY,
                body,
            });
        }
        app.update();
        app
    }

    #[test]
    fn five_trade_bodies_produce_one_event_each() {
        let app = drain(vec![
            Body::TradeRequestReceived(net::TradeRequestReceived {
                char_id: 5,
                name: "Alice".into(),
            }),
            Body::TradeOpened(net::TradeOpened {
                partner_char_id: 5,
                partner_name: "Alice".into(),
            }),
            Body::TradeOfferUpdate(net::TradeOfferUpdate {
                own_zeny: 20,
                ..Default::default()
            }),
            Body::TradeCompleted(net::TradeCompleted {}),
            Body::TradeCancelled(net::TradeCancelled { reason: 3 }),
        ]);
        let world = app.world();
        let requests: Vec<_> = world
            .resource::<Messages<TradeRequestNotified>>()
            .iter_current_update_messages()
            .collect();
        let opened: Vec<_> = world
            .resource::<Messages<TradeOpened>>()
            .iter_current_update_messages()
            .collect();
        let offers: Vec<_> = world
            .resource::<Messages<TradeOfferUpdated>>()
            .iter_current_update_messages()
            .collect();
        let completed: Vec<_> = world
            .resource::<Messages<TradeCompleted>>()
            .iter_current_update_messages()
            .collect();
        let cancelled: Vec<_> = world
            .resource::<Messages<TradeCancelled>>()
            .iter_current_update_messages()
            .collect();
        assert_eq!(
            (
                requests.len(),
                opened.len(),
                offers.len(),
                completed.len(),
                cancelled.len()
            ),
            (1, 1, 1, 1, 1)
        );
        assert_eq!(
            (requests[0].char_id, requests[0].name.as_str()),
            (5, "Alice")
        );
        assert_eq!(
            (opened[0].partner_char_id, opened[0].partner_name.as_str()),
            (5, "Alice")
        );
        assert_eq!(offers[0].own_zeny, 20);
        assert_eq!(
            cancelled[0].reason,
            net_contract::dto::TradeCancelReason::TooFar
        );
    }

    #[test]
    fn unrelated_body_produces_no_trade_events() {
        let app = drain(vec![Body::Announcement(net::Announcement::default())]);
        let world = app.world();
        assert!(
            world
                .resource::<Messages<TradeRequestNotified>>()
                .is_empty()
        );
        assert!(world.resource::<Messages<TradeOpened>>().is_empty());
        assert!(world.resource::<Messages<TradeOfferUpdated>>().is_empty());
        assert!(world.resource::<Messages<TradeCompleted>>().is_empty());
        assert!(world.resource::<Messages<TradeCancelled>>().is_empty());
    }
}
