//! Neutral vending notice text, shared by the seller and buyer windows.

use lifthrasir_client::domain::vending::VendingNotice;
use net_contract::dto::{
    VendingCloseOutcome, VendingListOutcome, VendingOpenOutcome, VendingPurchaseOutcome,
};

/// One line of player-facing feedback for a settled vending request.
pub fn vending_notice_text(notice: &VendingNotice) -> String {
    match notice {
        VendingNotice::Open { outcome, .. } => open_text(*outcome),
        VendingNotice::List { outcome, .. } => list_text(*outcome),
        VendingNotice::Purchase { outcome, .. } => purchase_text(*outcome),
        VendingNotice::Close { outcome, .. } => close_text(*outcome),
        VendingNotice::Unconfirmed { .. } => {
            "The server has not answered that vending request yet.".to_string()
        }
    }
}

fn open_text(outcome: VendingOpenOutcome) -> String {
    let text = match outcome {
        VendingOpenOutcome::Success => "Your shop is open.",
        VendingOpenOutcome::NoCart => "You need a pushcart to sell.",
        VendingOpenOutcome::SkillNotLearned => "You have not learned Vending.",
        VendingOpenOutcome::TooManySlots => "That is more listings than skill level allows.",
        VendingOpenOutcome::InvalidAmount => "A listed quantity is not valid.",
        VendingOpenOutcome::InvalidPrice => "A listed price is not valid.",
        VendingOpenOutcome::ItemNotInCart => "An item is no longer in your cart.",
        VendingOpenOutcome::InsufficientStock => "A listed quantity exceeds the cart stock.",
        VendingOpenOutcome::InvalidState => "You cannot open a shop right now.",
        VendingOpenOutcome::InsufficientSp => "You do not have enough SP.",
        VendingOpenOutcome::NpcNearby => "You cannot open a shop next to an NPC.",
        VendingOpenOutcome::MapRestricted => "Shops cannot be opened on this map.",
        VendingOpenOutcome::CellRestricted => "A shop cannot be opened here.",
        VendingOpenOutcome::InvalidTitle => "That shop title is not allowed.",
        VendingOpenOutcome::NoItems => "List at least one item.",
        VendingOpenOutcome::DuplicateSlot => "A cart slot is listed twice.",
        VendingOpenOutcome::ItemUnidentified => "An unidentified item cannot be sold.",
        VendingOpenOutcome::ItemBroken => "A broken item cannot be sold.",
        VendingOpenOutcome::ItemRental => "A rented item cannot be sold.",
        VendingOpenOutcome::ItemBound => "A bound item cannot be sold.",
        VendingOpenOutcome::ItemUntradeable => "An untradeable item cannot be sold.",
        VendingOpenOutcome::SetupExpired => "The shop setup expired. Cast Vending again.",
        VendingOpenOutcome::Forbidden => "You are not allowed to sell here.",
        VendingOpenOutcome::ZenyLimit => "The listed value is too high.",
        VendingOpenOutcome::PersistenceFailed => "The shop could not be saved. Try again.",
        VendingOpenOutcome::InvalidRequest => "The shop request was rejected.",
        VendingOpenOutcome::Unknown(code) => {
            return format!("Unexpected vending result ({code}).");
        }
    };
    text.to_string()
}

fn list_text(outcome: VendingListOutcome) -> String {
    let text = match outcome {
        VendingListOutcome::Success => "Shop stock loaded.",
        VendingListOutcome::NotOpen => "That shop is no longer open.",
        VendingListOutcome::ShopChanged => "That shop has changed. Look at it again.",
        VendingListOutcome::Forbidden => "You cannot trade with that shop.",
        VendingListOutcome::InvalidRequest => "The shop request was rejected.",
        VendingListOutcome::InternalError => "The server could not read that shop.",
        VendingListOutcome::Unknown(code) => {
            return format!("Unexpected vending result ({code}).");
        }
    };
    text.to_string()
}

fn purchase_text(outcome: VendingPurchaseOutcome) -> String {
    let text = match outcome {
        VendingPurchaseOutcome::Success => "Purchase complete.",
        VendingPurchaseOutcome::NotOpen => "That shop is no longer open.",
        VendingPurchaseOutcome::ShopChanged => "That shop has changed. Check it again.",
        VendingPurchaseOutcome::PriceChanged => "A price changed. Check the basket again.",
        VendingPurchaseOutcome::InsufficientStock => "Someone else bought that stock first.",
        VendingPurchaseOutcome::InvalidLine => "The basket refers to an item that is gone.",
        VendingPurchaseOutcome::InvalidAmount => "A basket quantity is not valid.",
        VendingPurchaseOutcome::NotEnoughZeny => "You do not have enough zeny.",
        VendingPurchaseOutcome::Overweight => "You cannot carry that much weight.",
        VendingPurchaseOutcome::NoSlots => "Your inventory has no free slots.",
        VendingPurchaseOutcome::StackLimit => "You cannot hold that many of those items.",
        VendingPurchaseOutcome::SellerZenyLimit => "The seller cannot hold that much zeny.",
        VendingPurchaseOutcome::Forbidden => "You are not allowed to buy from that shop.",
        VendingPurchaseOutcome::OutOfRange => "That vendor is too far away.",
        VendingPurchaseOutcome::WrongMap => "That vendor is on another map.",
        VendingPurchaseOutcome::SelfPurchase => "You cannot buy from your own shop.",
        VendingPurchaseOutcome::InvalidRequest => "The purchase request was rejected.",
        VendingPurchaseOutcome::PersistenceFailed => "The purchase could not be saved. Try again.",
        VendingPurchaseOutcome::InternalError => "The server could not complete the purchase.",
        VendingPurchaseOutcome::StaleRequest => {
            "That purchase request is stale. Check the shop again."
        }
        VendingPurchaseOutcome::InvalidState => "You cannot buy right now.",
        VendingPurchaseOutcome::Unknown(code) => {
            return format!("Unexpected vending result ({code}).");
        }
    };
    text.to_string()
}

fn close_text(outcome: VendingCloseOutcome) -> String {
    let text = match outcome {
        VendingCloseOutcome::ShopClosed => "Your shop is closed.",
        VendingCloseOutcome::SetupCanceled => "Shop setup canceled.",
        VendingCloseOutcome::AlreadyClosed => "That shop was already closed.",
        VendingCloseOutcome::InvalidTarget => "That shop instance no longer exists.",
        VendingCloseOutcome::Forbidden => "You cannot change that shop.",
        VendingCloseOutcome::InvalidRequest => "The close request was rejected.",
        VendingCloseOutcome::InternalError => "The server could not close the shop.",
        VendingCloseOutcome::Unknown(code) => {
            return format!("Unexpected vending result ({code}).");
        }
    };
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_client::domain::vending::VendingNotice;
    use net_contract::dto::VendingCloseTarget;

    fn open(outcome: VendingOpenOutcome) -> VendingNotice {
        VendingNotice::Open {
            request_id: 1,
            setup_id: "setup-1".into(),
            outcome,
        }
    }

    fn purchase(outcome: VendingPurchaseOutcome) -> VendingNotice {
        VendingNotice::Purchase {
            request_id: 2,
            vendor_unit_id: 5,
            shop_id: "shop-1".into(),
            outcome,
        }
    }

    fn close(outcome: VendingCloseOutcome) -> VendingNotice {
        VendingNotice::Close {
            request_id: 3,
            target: VendingCloseTarget::Shop("shop-1".into()),
            outcome,
        }
    }

    #[test]
    fn known_outcomes_read_as_distinct_sentences() {
        assert_eq!(
            vending_notice_text(&open(VendingOpenOutcome::Success)),
            "Your shop is open."
        );
        assert_eq!(
            vending_notice_text(&open(VendingOpenOutcome::ItemBound)),
            "A bound item cannot be sold."
        );
        assert_eq!(
            vending_notice_text(&purchase(VendingPurchaseOutcome::NotEnoughZeny)),
            "You do not have enough zeny."
        );
        assert_eq!(
            vending_notice_text(&close(VendingCloseOutcome::SetupCanceled)),
            "Shop setup canceled."
        );
        assert_eq!(
            vending_notice_text(&VendingNotice::List {
                request_id: 4,
                vendor_unit_id: 5,
                outcome: VendingListOutcome::ShopChanged,
            }),
            "That shop has changed. Look at it again."
        );
    }

    #[test]
    fn unknown_outcomes_and_unconfirmed_state_are_reported_verbatim() {
        assert_eq!(
            vending_notice_text(&open(VendingOpenOutcome::Unknown(91))),
            "Unexpected vending result (91)."
        );
        assert_eq!(
            vending_notice_text(&purchase(VendingPurchaseOutcome::Unknown(0))),
            "Unexpected vending result (0)."
        );
        assert_eq!(
            vending_notice_text(&close(VendingCloseOutcome::Unknown(-7))),
            "Unexpected vending result (-7)."
        );
        assert_eq!(
            vending_notice_text(&VendingNotice::Unconfirmed { request_id: 9 }),
            "The server has not answered that vending request yet."
        );
    }

    #[test]
    fn every_success_is_worded_differently_from_every_failure() {
        let success = [
            vending_notice_text(&open(VendingOpenOutcome::Success)),
            vending_notice_text(&purchase(VendingPurchaseOutcome::Success)),
            vending_notice_text(&close(VendingCloseOutcome::ShopClosed)),
        ];
        for (index, one) in success.iter().enumerate() {
            for other in success.iter().skip(index + 1) {
                assert_ne!(one, other);
            }
            for failure in [
                vending_notice_text(&open(VendingOpenOutcome::InvalidState)),
                vending_notice_text(&purchase(VendingPurchaseOutcome::InvalidState)),
                vending_notice_text(&close(VendingCloseOutcome::InvalidRequest)),
            ] {
                assert_ne!(*one, failure);
            }
        }
    }
}
