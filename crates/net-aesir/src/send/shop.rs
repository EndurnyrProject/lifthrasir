use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{BuyFromShop, CloseNpcShop, SellToShop};

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::{
    NpcBuyEntry, NpcBuyRequest, NpcSellEntry, NpcSellRequest, NpcShopClose,
};
use crate::zone::{QuicZoneState, ZonePhase};

fn buy_body(c: &BuyFromShop) -> Body {
    Body::NpcBuyRequest(NpcBuyRequest {
        unit_id: c.unit_id,
        items: c
            .items
            .iter()
            .map(|e| NpcBuyEntry {
                nameid: e.nameid,
                amount: e.amount,
            })
            .collect(),
    })
}

fn sell_body(c: &SellToShop) -> Body {
    Body::NpcSellRequest(NpcSellRequest {
        unit_id: c.unit_id,
        items: c
            .items
            .iter()
            .map(|e| NpcSellEntry {
                inventory_index: e.inventory_index,
                amount: e.amount,
            })
            .collect(),
    })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_shop_buy(
    mut events: MessageReader<BuyFromShop>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, buy_body(ev)) {
            error!("failed to send NpcBuyRequest: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_shop_sell(
    mut events: MessageReader<SellToShop>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, sell_body(ev)) {
            error!("failed to send NpcSellRequest: {e}");
        }
    }
}

fn close_shop_body(command: &CloseNpcShop) -> Body {
    Body::NpcShopClose(NpcShopClose {
        unit_id: command.unit_id,
    })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_shop_close(
    mut events: MessageReader<CloseNpcShop>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for event in events.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, close_shop_body(event)) {
            error!("failed to send NpcShopClose: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::{BuyEntry, SellEntry};

    #[test]
    fn close_preserves_the_full_shop_identity() {
        let unit_id = u64::MAX - 1;
        assert!(matches!(close_shop_body(&CloseNpcShop { unit_id }),
            Body::NpcShopClose(close) if close.unit_id == unit_id));
    }

    #[test]
    fn buy_body_maps_unit_id_and_entries() {
        let body = buy_body(&BuyFromShop {
            unit_id: 42,
            items: vec![BuyEntry {
                nameid: 501,
                amount: 3,
            }],
        });
        match body {
            Body::NpcBuyRequest(NpcBuyRequest { unit_id, items }) => {
                assert_eq!(unit_id, 42u64);
                assert_eq!(
                    items,
                    vec![NpcBuyEntry {
                        nameid: 501,
                        amount: 3
                    }]
                );
            }
            other => panic!("expected Body::NpcBuyRequest, got {other:?}"),
        }
    }

    #[test]
    fn sell_body_maps_unit_id_and_entries() {
        let body = sell_body(&SellToShop {
            unit_id: 7,
            items: vec![SellEntry {
                inventory_index: 2,
                amount: 5,
            }],
        });
        match body {
            Body::NpcSellRequest(NpcSellRequest { unit_id, items }) => {
                assert_eq!(unit_id, 7u64);
                assert_eq!(
                    items,
                    vec![NpcSellEntry {
                        inventory_index: 2,
                        amount: 5
                    }]
                );
            }
            other => panic!("expected Body::NpcSellRequest, got {other:?}"),
        }
    }
}
