use bevy::prelude::warn;
use net_contract::dto::TradeCancelReason;
use net_contract::events::{TradeCancelled, TradeOfferUpdated, TradeOpened, TradeRequestNotified};

use super::inventory::inventory_item;
use crate::proto::aesir::net;

pub fn trade_request_received(message: net::TradeRequestReceived) -> TradeRequestNotified {
    TradeRequestNotified {
        char_id: message.char_id,
        name: message.name,
    }
}

pub fn trade_opened(message: net::TradeOpened) -> TradeOpened {
    TradeOpened {
        partner_char_id: message.partner_char_id,
        partner_name: message.partner_name,
    }
}

pub fn trade_offer_update(message: net::TradeOfferUpdate) -> TradeOfferUpdated {
    TradeOfferUpdated {
        own: message.own.into_iter().map(inventory_item).collect(),
        partner: message.partner.into_iter().map(inventory_item).collect(),
        own_zeny: message.own_zeny,
        partner_zeny: message.partner_zeny,
        own_locked: message.own_locked,
        partner_locked: message.partner_locked,
    }
}

pub fn cancel_reason(raw: i32) -> TradeCancelReason {
    use net::TradeCancelReason as Wire;
    match Wire::try_from(raw) {
        Ok(Wire::Declined) => TradeCancelReason::Declined,
        Ok(Wire::Timeout) => TradeCancelReason::Timeout,
        Ok(Wire::Cancelled) => TradeCancelReason::Cancelled,
        Ok(Wire::TooFar) => TradeCancelReason::TooFar,
        Ok(Wire::Busy) => TradeCancelReason::Busy,
        Ok(Wire::Dead) => TradeCancelReason::Dead,
        Ok(Wire::Disconnected) => TradeCancelReason::Disconnected,
        Ok(Wire::Capacity) => TradeCancelReason::Capacity,
        Ok(Wire::Invalid) => TradeCancelReason::Invalid,
        Err(_) => {
            warn!(raw, "unknown trade cancellation reason");
            TradeCancelReason::Unknown(raw)
        }
    }
}

pub fn trade_cancelled(message: net::TradeCancelled) -> TradeCancelled {
    TradeCancelled {
        reason: cancel_reason(message.reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_known_and_unknown_cancel_reasons_are_distinct() {
        let cases = [
            (
                net::TradeCancelReason::Declined,
                TradeCancelReason::Declined,
            ),
            (net::TradeCancelReason::Timeout, TradeCancelReason::Timeout),
            (
                net::TradeCancelReason::Cancelled,
                TradeCancelReason::Cancelled,
            ),
            (net::TradeCancelReason::TooFar, TradeCancelReason::TooFar),
            (net::TradeCancelReason::Busy, TradeCancelReason::Busy),
            (net::TradeCancelReason::Dead, TradeCancelReason::Dead),
            (
                net::TradeCancelReason::Disconnected,
                TradeCancelReason::Disconnected,
            ),
            (
                net::TradeCancelReason::Capacity,
                TradeCancelReason::Capacity,
            ),
            (net::TradeCancelReason::Invalid, TradeCancelReason::Invalid),
        ];
        for (raw, expected) in cases {
            assert_eq!(cancel_reason(raw as i32), expected);
        }
        assert_eq!(cancel_reason(999), TradeCancelReason::Unknown(999));
    }

    #[test]
    fn offer_update_preserves_inventory_fields_and_both_sides() {
        let own = net::InventoryItem {
            index: 70_000,
            nameid: 501,
            amount: 80_000,
            refine: 7,
            identified: true,
            ..Default::default()
        };
        let partner = net::InventoryItem {
            index: 0,
            nameid: 502,
            amount: 2,
            refine: 3,
            identified: false,
            ..Default::default()
        };
        let update = trade_offer_update(net::TradeOfferUpdate {
            own: vec![own],
            partner: vec![partner],
            own_zeny: u32::MAX as u64 + 1,
            partner_zeny: 200,
            own_locked: true,
            partner_locked: false,
        });
        assert_eq!(
            (
                update.own[0].index,
                update.own[0].nameid,
                update.own[0].amount,
                update.own[0].refine,
                update.own[0].identified
            ),
            (70_000, 501, 80_000, 7, true)
        );
        assert_eq!(
            (
                update.partner[0].index,
                update.partner[0].nameid,
                update.partner[0].amount,
                update.partner[0].refine,
                update.partner[0].identified
            ),
            (0, 502, 2, 3, false)
        );
        assert_eq!(
            (update.own_zeny, update.partner_zeny),
            (u32::MAX as u64 + 1, 200)
        );
        assert!(update.own_locked);
        assert!(!update.partner_locked);
    }
}
