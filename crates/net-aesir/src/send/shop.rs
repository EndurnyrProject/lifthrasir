use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{BuyFromShop, SellToShop};

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::{NpcBuyEntry, NpcBuyRequest, NpcSellEntry, NpcSellRequest};
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

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::{BuyEntry, SellEntry};

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
