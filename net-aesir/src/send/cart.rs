use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{MountCart, MoveFromCart, MoveToCart};

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::{CartMountRequest, MoveFromCartRequest, MoveToCartRequest};
use crate::zone::{QuicZoneState, ZonePhase};

fn mount_body(c: &MountCart) -> Body {
    Body::CartMountRequest(CartMountRequest { mount: c.mount })
}

fn move_to_cart_body(c: &MoveToCart) -> Body {
    Body::MoveToCartRequest(MoveToCartRequest {
        inventory_index: c.inventory_index as u32,
        amount: c.amount as u32,
    })
}

fn move_from_cart_body(c: &MoveFromCart) -> Body {
    Body::MoveFromCartRequest(MoveFromCartRequest {
        cart_index: c.cart_index as u32,
        amount: c.amount as u32,
    })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_mount_cart(
    mut events: MessageReader<MountCart>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, mount_body(ev)) {
            error!("failed to send CartMountRequest: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_move_to_cart(
    mut events: MessageReader<MoveToCart>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, move_to_cart_body(ev)) {
            error!("failed to send MoveToCartRequest: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_move_from_cart(
    mut events: MessageReader<MoveFromCart>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, move_from_cart_body(ev)) {
            error!("failed to send MoveFromCartRequest: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_body_carries_mount_flag() {
        let body = mount_body(&MountCart { mount: true });
        match body {
            Body::CartMountRequest(CartMountRequest { mount }) => assert!(mount),
            other => panic!("expected Body::CartMountRequest, got {other:?}"),
        }
    }

    #[test]
    fn move_to_cart_body_widens_indices_to_u32() {
        let body = move_to_cart_body(&MoveToCart {
            inventory_index: 4,
            amount: 2,
        });
        match body {
            Body::MoveToCartRequest(MoveToCartRequest {
                inventory_index,
                amount,
            }) => {
                assert_eq!(inventory_index, 4u32);
                assert_eq!(amount, 2u32);
            }
            other => panic!("expected Body::MoveToCartRequest, got {other:?}"),
        }
    }

    #[test]
    fn move_from_cart_body_widens_indices_to_u32() {
        let body = move_from_cart_body(&MoveFromCart {
            cart_index: 7,
            amount: 3,
        });
        match body {
            Body::MoveFromCartRequest(MoveFromCartRequest { cart_index, amount }) => {
                assert_eq!(cart_index, 7u32);
                assert_eq!(amount, 3u32);
            }
            other => panic!("expected Body::MoveFromCartRequest, got {other:?}"),
        }
    }
}
