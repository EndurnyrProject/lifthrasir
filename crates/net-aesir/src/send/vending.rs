//! Outbound vending commands: neutral intents translated to wire requests.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{
    CloseVendingRequested, ListVendingRequested, OpenVendingRequested, PurchaseVendingRequested,
};
use net_contract::dto::{VendingCloseTarget, VendingOpenOutcome};
use net_contract::events::{VendingIngress, VendingIngressPayload};
use net_contract::state::ZoneSessionGeneration;

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net;
use crate::zone::mapping::vending::raw_cart_index;
use crate::zone::{QuicZoneState, ZonePhase};

/// Builds the open request, rejecting offers whose neutral cart index cannot
/// exist in the server's cart space instead of wrapping around.
pub fn open_vending_body(command: &OpenVendingRequested) -> Result<Body, &'static str> {
    let entries = command
        .entries
        .iter()
        .map(|offer| {
            let cart_index = raw_cart_index(offer.cart_index)
                .ok_or("offer cart index is below the server cart offset")?;
            Ok(net::VendingEntry {
                cart_index,
                amount: offer.amount,
                price: offer.price,
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    Ok(Body::VendingOpenRequest(net::VendingOpenRequest {
        title: command.title.clone(),
        entries,
        request_id: command.request_id,
        setup_id: command.setup_id.clone(),
    }))
}

pub fn list_vending_body(command: &ListVendingRequested) -> Body {
    Body::VendingListRequest(net::VendingListRequest {
        vendor_unit_id: command.vendor_unit_id,
        request_id: command.request_id,
        expected_shop_id: command.expected_shop_id.clone().unwrap_or_default(),
    })
}

pub fn purchase_vending_body(command: &PurchaseVendingRequested) -> Body {
    Body::VendingPurchaseRequest(net::VendingPurchaseRequest {
        vendor_unit_id: command.vendor_unit_id,
        items: command
            .items
            .iter()
            .map(|line| net::VendingBuy {
                index: line.index,
                amount: line.amount,
                expected_unit_price: line.expected_unit_price,
            })
            .collect(),
        request_id: command.request_id,
        shop_id: command.shop_id.clone(),
    })
}

pub fn close_vending_body(command: &CloseVendingRequested) -> Body {
    Body::VendingCloseRequest(net::VendingCloseRequest {
        request_id: command.request_id,
        target: Some(match &command.target {
            VendingCloseTarget::Setup(id) => {
                net::vending_close_request::Target::SetupId(id.clone())
            }
            VendingCloseTarget::Shop(id) => net::vending_close_request::Target::ShopId(id.clone()),
        }),
    })
}

#[derive(SystemParam)]
pub struct VendingCommandQueues<'w, 's> {
    opens: MessageReader<'w, 's, OpenVendingRequested>,
    lists: MessageReader<'w, 's, ListVendingRequested>,
    purchases: MessageReader<'w, 's, PurchaseVendingRequested>,
    closes: MessageReader<'w, 's, CloseVendingRequested>,
}

impl VendingCommandQueues<'_, '_> {
    fn clear_all(&mut self) {
        self.opens.clear();
        self.lists.clear();
        self.purchases.clear();
        self.closes.clear();
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Last,
    config(run_if = not(client_connected))
)]
pub fn clear_vending_commands_while_disconnected(
    mut opens: ResMut<Messages<OpenVendingRequested>>,
    mut lists: ResMut<Messages<ListVendingRequested>>,
    mut purchases: ResMut<Messages<PurchaseVendingRequested>>,
    mut closes: ResMut<Messages<CloseVendingRequested>>,
) {
    opens.clear();
    lists.clear();
    purchases.clear();
    closes.clear();
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_vending_commands(
    mut queues: VendingCommandQueues,
    generation: Res<ZoneSessionGeneration>,
    mut ingress: MessageWriter<VendingIngress>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        queues.clear_all();
        return;
    }

    for command in queues.opens.read() {
        match open_vending_body(command) {
            Ok(body) => {
                if let Err(error) = zone.send(&mut client, GAMEPLAY, body) {
                    error!("failed to send VendingOpenRequest: {error}");
                }
            }
            Err(reason) => {
                warn!("rejecting local vending open: {reason}");
                ingress.write(VendingIngress {
                    generation: *generation,
                    payload: VendingIngressPayload::OpenResult {
                        request_id: command.request_id,
                        setup_id: command.setup_id.clone(),
                        setup_active: true,
                        outcome: VendingOpenOutcome::InvalidRequest,
                        snapshot: None,
                    },
                });
            }
        }
    }

    let bodies = queues
        .lists
        .read()
        .map(list_vending_body)
        .chain(queues.purchases.read().map(purchase_vending_body))
        .chain(queues.closes.read().map(close_vending_body));
    for body in bodies {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, body) {
            error!("failed to send vending command: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::{VendingBuyLine, VendingOffer};

    fn open_command(entries: Vec<VendingOffer>) -> OpenVendingRequested {
        OpenVendingRequested {
            request_id: 4,
            setup_id: "setup-1".into(),
            title: "Potions".into(),
            entries,
        }
    }

    #[test]
    fn offers_translate_neutral_indices_to_raw_slots() {
        let body = open_vending_body(&open_command(vec![
            VendingOffer {
                cart_index: 2,
                amount: 1,
                price: 0,
            },
            VendingOffer {
                cart_index: 9,
                amount: 3,
                price: 1_000_000_000,
            },
        ]))
        .expect("open body");

        assert!(matches!(
            body,
            Body::VendingOpenRequest(net::VendingOpenRequest { title, entries, request_id: 4, setup_id })
                if title == "Potions"
                    && setup_id == "setup-1"
                    && entries.len() == 2
                    && entries[0].cart_index == 0
                    && entries[1].cart_index == 7
                    && entries[1].amount == 3
                    && entries[1].price == 1_000_000_000
        ));
    }

    #[test]
    fn impossible_offer_index_is_rejected_not_wrapped() {
        for cart_index in [0, 1] {
            assert!(
                open_vending_body(&open_command(vec![VendingOffer {
                    cart_index,
                    amount: 1,
                    price: 1,
                }]))
                .is_err()
            );
        }
    }

    #[test]
    fn list_and_purchase_keep_opaque_listing_identity() {
        let list = list_vending_body(&ListVendingRequested {
            request_id: 5,
            vendor_unit_id: 77,
            expected_shop_id: None,
        });
        assert!(matches!(
            list,
            Body::VendingListRequest(net::VendingListRequest {
                vendor_unit_id: 77,
                request_id: 5,
                expected_shop_id
            }) if expected_shop_id.is_empty()
        ));

        let list = list_vending_body(&ListVendingRequested {
            request_id: 6,
            vendor_unit_id: 78,
            expected_shop_id: Some("shop-2".into()),
        });
        assert!(matches!(
            list,
            Body::VendingListRequest(net::VendingListRequest { expected_shop_id, .. })
                if expected_shop_id == "shop-2"
        ));

        let purchase = purchase_vending_body(&PurchaseVendingRequested {
            request_id: 7,
            vendor_unit_id: 77,
            shop_id: "shop-1".into(),
            items: vec![VendingBuyLine {
                index: 3,
                amount: 2,
                expected_unit_price: 500,
            }],
        });
        assert!(matches!(
            purchase,
            Body::VendingPurchaseRequest(net::VendingPurchaseRequest { request_id: 7, vendor_unit_id: 77, shop_id, items })
                if shop_id == "shop-1"
                    && items.len() == 1
                    && items[0].index == 3
                    && items[0].amount == 2
                    && items[0].expected_unit_price == 500
        ));
    }

    #[test]
    fn close_target_keeps_the_attempted_identity() {
        let setup = close_vending_body(&CloseVendingRequested {
            request_id: 8,
            target: VendingCloseTarget::Setup("setup-1".into()),
        });
        assert!(matches!(
            setup,
            Body::VendingCloseRequest(net::VendingCloseRequest {
                request_id: 8,
                target: Some(net::vending_close_request::Target::SetupId(id))
            }) if id == "setup-1"
        ));

        let shop = close_vending_body(&CloseVendingRequested {
            request_id: 9,
            target: VendingCloseTarget::Shop("shop-1".into()),
        });
        assert!(matches!(
            shop,
            Body::VendingCloseRequest(net::VendingCloseRequest {
                target: Some(net::vending_close_request::Target::ShopId(id)), ..
            }) if id == "shop-1"
        ));
    }

    fn app_with_commands() -> App {
        let mut app = App::new();
        app.init_resource::<QuinnetClient>()
            .insert_resource(QuicZoneState {
                phase: ZonePhase::Playing,
                ..Default::default()
            })
            .insert_resource(ZoneSessionGeneration(3))
            .add_message::<OpenVendingRequested>()
            .add_message::<ListVendingRequested>()
            .add_message::<PurchaseVendingRequested>()
            .add_message::<CloseVendingRequested>()
            .add_message::<VendingIngress>();
        app
    }

    fn queue_all(app: &mut App) {
        let world = app.world_mut();
        world.write_message(open_command(vec![VendingOffer {
            cart_index: 2,
            amount: 1,
            price: 5,
        }]));
        world.write_message(ListVendingRequested {
            request_id: 5,
            vendor_unit_id: 77,
            expected_shop_id: Some("shop-1".into()),
        });
        world.write_message(PurchaseVendingRequested {
            request_id: 6,
            vendor_unit_id: 77,
            shop_id: "shop-1".into(),
            items: vec![],
        });
        world.write_message(CloseVendingRequested {
            request_id: 7,
            target: VendingCloseTarget::Shop("shop-1".into()),
        });
    }

    #[test]
    fn impossible_index_reports_a_correlated_local_rejection() {
        let mut app = app_with_commands();
        app.add_systems(Update, send_vending_commands);
        app.world_mut()
            .write_message(open_command(vec![VendingOffer {
                cart_index: 1,
                amount: 1,
                price: 5,
            }]));
        app.update();

        let ingress = app.world().resource::<Messages<VendingIngress>>();
        let events: Vec<_> = ingress.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].generation, ZoneSessionGeneration(3));
        assert_eq!(
            events[0].payload,
            VendingIngressPayload::OpenResult {
                request_id: 4,
                setup_id: "setup-1".into(),
                setup_active: true,
                outcome: VendingOpenOutcome::InvalidRequest,
                snapshot: None,
            }
        );
    }

    #[test]
    fn disconnected_commands_are_cleared() {
        let mut app = app_with_commands();
        app.add_systems(Last, clear_vending_commands_while_disconnected);
        queue_all(&mut app);
        app.update();
        let world = app.world();
        assert!(
            world
                .resource::<Messages<OpenVendingRequested>>()
                .is_empty()
        );
        assert!(
            world
                .resource::<Messages<ListVendingRequested>>()
                .is_empty()
        );
        assert!(
            world
                .resource::<Messages<PurchaseVendingRequested>>()
                .is_empty()
        );
        assert!(
            world
                .resource::<Messages<CloseVendingRequested>>()
                .is_empty()
        );
    }

    #[test]
    fn out_of_phase_commands_are_consumed_without_ingress() {
        let mut app = app_with_commands();
        app.world_mut().resource_mut::<QuicZoneState>().phase = ZonePhase::Disconnected;
        app.add_systems(Update, send_vending_commands);
        queue_all(&mut app);
        app.update();
        assert!(
            app.world()
                .resource::<Messages<VendingIngress>>()
                .is_empty()
        );
    }
}
