use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{
    AddTradeItem, CancelTrade, ConfirmTrade, LockTrade, RemoveTradeItem, RequestTrade,
    RespondTrade, SetTradeZeny,
};

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net;
use crate::zone::{QuicZoneState, ZonePhase};

fn request_body(command: &RequestTrade) -> Body {
    Body::TradeRequest(net::TradeRequest {
        target_gid: command.target_char_id,
    })
}

fn response_body(command: &RespondTrade) -> Body {
    Body::TradeResponse(net::TradeResponse {
        accept: command.accept,
    })
}

fn add_item_body(command: &AddTradeItem) -> Body {
    Body::TradeAddItem(net::TradeAddItem {
        index: command.index,
        amount: command.amount,
    })
}

fn remove_item_body(command: &RemoveTradeItem) -> Body {
    Body::TradeRemoveItem(net::TradeRemoveItem {
        index: command.index,
    })
}

fn set_zeny_body(command: &SetTradeZeny) -> Body {
    Body::TradeSetZeny(net::TradeSetZeny {
        amount: command.amount,
    })
}

fn lock_body(_: &LockTrade) -> Body {
    Body::TradeLock(net::TradeLock {})
}

fn confirm_body(_: &ConfirmTrade) -> Body {
    Body::TradeConfirm(net::TradeConfirm {})
}

fn cancel_body(_: &CancelTrade) -> Body {
    Body::TradeCancel(net::TradeCancel {})
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Last,
    config(run_if = not(client_connected))
)]
#[expect(clippy::too_many_arguments, reason = "one buffer per trade command")]
pub fn clear_trade_commands_while_disconnected(
    mut requests: ResMut<Messages<RequestTrade>>,
    mut responses: ResMut<Messages<RespondTrade>>,
    mut adds: ResMut<Messages<AddTradeItem>>,
    mut removes: ResMut<Messages<RemoveTradeItem>>,
    mut zeny: ResMut<Messages<SetTradeZeny>>,
    mut locks: ResMut<Messages<LockTrade>>,
    mut confirms: ResMut<Messages<ConfirmTrade>>,
    mut cancels: ResMut<Messages<CancelTrade>>,
) {
    requests.clear();
    responses.clear();
    adds.clear();
    removes.clear();
    zeny.clear();
    locks.clear();
    confirms.clear();
    cancels.clear();
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
#[expect(
    clippy::too_many_arguments,
    reason = "keep zeny before lock in one send system"
)]
pub fn send_trade_commands(
    mut requests: MessageReader<RequestTrade>,
    mut responses: MessageReader<RespondTrade>,
    mut adds: MessageReader<AddTradeItem>,
    mut removes: MessageReader<RemoveTradeItem>,
    mut zeny: MessageReader<SetTradeZeny>,
    mut locks: MessageReader<LockTrade>,
    mut confirms: MessageReader<ConfirmTrade>,
    mut cancels: MessageReader<CancelTrade>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        requests.clear();
        responses.clear();
        adds.clear();
        removes.clear();
        zeny.clear();
        locks.clear();
        confirms.clear();
        cancels.clear();
        return;
    }

    // Zeny must be sent before Lock when both commands are queued in one frame.
    let bodies = requests
        .read()
        .map(request_body)
        .chain(responses.read().map(response_body))
        .chain(adds.read().map(add_item_body))
        .chain(removes.read().map(remove_item_body))
        .chain(zeny.read().map(set_zeny_body))
        .chain(locks.read().map(lock_body))
        .chain(confirms.read().map(confirm_body))
        .chain(cancels.read().map(cancel_body));
    for body in bodies {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, body) {
            error!("failed to send trade command: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_trade_bodies_preserve_fields_and_variants() {
        assert!(matches!(
            request_body(&RequestTrade { target_char_id: 42 }),
            Body::TradeRequest(net::TradeRequest { target_gid: 42 })
        ));
        assert!(matches!(
            response_body(&RespondTrade { accept: true }),
            Body::TradeResponse(net::TradeResponse { accept: true })
        ));
        assert!(matches!(
            add_item_body(&AddTradeItem {
                index: 70_000,
                amount: 80_000
            }),
            Body::TradeAddItem(net::TradeAddItem {
                index: 70_000,
                amount: 80_000
            })
        ));
        assert!(matches!(
            remove_item_body(&RemoveTradeItem { index: 70_001 }),
            Body::TradeRemoveItem(net::TradeRemoveItem { index: 70_001 })
        ));
        assert!(matches!(
            set_zeny_body(&SetTradeZeny {
                amount: u32::MAX as u64 + 1
            }),
            Body::TradeSetZeny(net::TradeSetZeny { amount }) if amount == u32::MAX as u64 + 1
        ));
        assert!(matches!(lock_body(&LockTrade), Body::TradeLock(_)));
        assert!(matches!(confirm_body(&ConfirmTrade), Body::TradeConfirm(_)));
        assert!(matches!(cancel_body(&CancelTrade), Body::TradeCancel(_)));
    }

    fn app_with_commands() -> App {
        let mut app = App::new();
        app.init_resource::<QuinnetClient>()
            .init_resource::<QuicZoneState>()
            .add_message::<RequestTrade>()
            .add_message::<RespondTrade>()
            .add_message::<AddTradeItem>()
            .add_message::<RemoveTradeItem>()
            .add_message::<SetTradeZeny>()
            .add_message::<LockTrade>()
            .add_message::<ConfirmTrade>()
            .add_message::<CancelTrade>();
        app
    }

    fn queue_all(app: &mut App) {
        let world = app.world_mut();
        world.write_message(RequestTrade { target_char_id: 42 });
        world.write_message(RespondTrade { accept: false });
        world.write_message(AddTradeItem {
            index: 2,
            amount: 1,
        });
        world.write_message(RemoveTradeItem { index: 2 });
        world.write_message(SetTradeZeny { amount: 100 });
        world.write_message(LockTrade);
        world.write_message(ConfirmTrade);
        world.write_message(CancelTrade);
    }

    #[test]
    fn disconnected_commands_are_cleared() {
        let mut app = app_with_commands();
        app.add_systems(Last, clear_trade_commands_while_disconnected);
        queue_all(&mut app);
        app.update();
        let world = app.world();
        assert!(world.resource::<Messages<RequestTrade>>().is_empty());
        assert!(world.resource::<Messages<RespondTrade>>().is_empty());
        assert!(world.resource::<Messages<AddTradeItem>>().is_empty());
        assert!(world.resource::<Messages<RemoveTradeItem>>().is_empty());
        assert!(world.resource::<Messages<SetTradeZeny>>().is_empty());
        assert!(world.resource::<Messages<LockTrade>>().is_empty());
        assert!(world.resource::<Messages<ConfirmTrade>>().is_empty());
        assert!(world.resource::<Messages<CancelTrade>>().is_empty());
    }

    #[test]
    fn out_of_phase_commands_are_consumed() {
        let mut app = app_with_commands();
        app.add_systems(Update, send_trade_commands);
        queue_all(&mut app);
        app.update();
        app.world_mut().resource_mut::<QuicZoneState>().phase = ZonePhase::Playing;
        app.update();
    }
}
