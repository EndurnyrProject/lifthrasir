//! Vending wire records mapped to the neutral contract.
//!
//! Aesir publishes cart slots two below the ordinary cart-packet index, so the
//! offset lives here and nowhere else: offer indices subtract it, setup grants
//! add it, and buyer listing indices are opaque on both sides.

use bevy::prelude::warn;
use net_contract::dto::{
    VendingBoard, VendingCloseOutcome, VendingCloseTarget, VendingClosureReason,
    VendingCreatorKind, VendingListOutcome, VendingListing, VendingOpenOutcome,
    VendingPurchaseOutcome, VendingSetup, VendingSnapshot,
};
use net_contract::events::VendingIngressPayload;

use super::inventory::item_options;
use crate::envelope::Body;
use crate::proto::aesir::net;

/// Difference between the ordinary cart-packet index and the raw server slot.
const CART_INDEX_OFFSET: u32 = 2;

/// Raw Aesir cart slot for a neutral cart index, or `None` when it cannot exist.
pub fn raw_cart_index(neutral: u32) -> Option<u32> {
    neutral.checked_sub(CART_INDEX_OFFSET)
}

/// Neutral cart index for a raw Aesir cart slot, or `None` when it cannot exist.
fn neutral_cart_index(raw: u32) -> Option<u32> {
    raw.checked_add(CART_INDEX_OFFSET)
}

pub fn vending_payload(body: Body) -> Option<VendingIngressPayload> {
    Some(match body {
        Body::VendingSetupReady(s) => VendingIngressPayload::SetupReady(VendingSetup {
            setup_id: s.setup_id,
            max_slots: s.max_slots,
            min_unit_price: s.min_unit_price,
            max_unit_price: s.max_unit_price,
            max_title_bytes: s.max_title_bytes,
            sellable_cart_indices: sellable_cart_indices(s.sellable_cart_indices),
            sp_spent: s.sp_spent,
        }),
        Body::VendingOpenResult(r) => {
            let net::VendingOpenResult {
                result,
                request_id,
                setup_id,
                setup_active,
                shop_id,
                title,
                items,
                vendor_unit_id,
            } = r;
            let outcome = open_outcome(result);
            VendingIngressPayload::OpenResult {
                request_id,
                setup_id,
                setup_active,
                snapshot: matches!(outcome, VendingOpenOutcome::Success).then_some(
                    VendingSnapshot {
                        vendor_unit_id,
                        shop_id,
                        title,
                        items: listings(items),
                        total_earned: 0,
                    },
                ),
                outcome,
            }
        }
        Body::VendingList(l) => {
            let outcome = list_outcome(l.result);
            VendingIngressPayload::ListResult {
                request_id: l.request_id,
                vendor_unit_id: l.vendor_unit_id,
                snapshot: matches!(outcome, VendingListOutcome::Success).then_some(
                    VendingSnapshot {
                        vendor_unit_id: l.vendor_unit_id,
                        shop_id: l.shop_id,
                        title: l.title,
                        items: listings(l.items),
                        total_earned: l.total_earned,
                    },
                ),
                outcome,
            }
        }
        Body::VendingPurchaseResult(p) => VendingIngressPayload::PurchaseResult {
            request_id: p.request_id,
            vendor_unit_id: p.vendor_unit_id,
            shop_id: p.shop_id,
            outcome: purchase_outcome(p.result),
            total_charged: p.total_charged,
        },
        Body::VendingCloseResult(c) => {
            let Some(target) = close_result_target(c.target) else {
                warn!("dropping vending close result without a target");
                return None;
            };
            VendingIngressPayload::CloseResult {
                request_id: c.request_id,
                target,
                outcome: close_outcome(c.result),
            }
        }
        Body::VendingBoardShown(b) => VendingIngressPayload::BoardShown(VendingBoard {
            vendor_unit_id: b.unit_id,
            shop_id: b.shop_id,
            title: b.title,
        }),
        Body::VendingBoardRemoved(b) => VendingIngressPayload::BoardRemoved {
            unit_id: b.unit_id,
            shop_id: b.shop_id,
            reason: closure_reason(b.reason),
        },
        Body::VendingSaleReport(s) => VendingIngressPayload::SaleReport {
            shop_id: s.shop_id,
            index: s.index,
            nameid: s.nameid,
            amount: s.amount,
            zeny_gained: s.zeny_gained,
            buyer_name: s.buyer_name,
            sale_sequence: s.sale_sequence,
            total_earned: s.total_earned,
        },
        Body::UnitDespawn(d) => VendingIngressPayload::VendorDespawned {
            unit_id: d.gid as u64,
        },
        Body::MapMove(_) => VendingIngressPayload::MapChanging,
        _ => return None,
    })
}

fn sellable_cart_indices(raw: Vec<u32>) -> Vec<u32> {
    raw.into_iter()
        .filter_map(|raw| {
            let neutral = neutral_cart_index(raw);
            if neutral.is_none() {
                warn!("dropping vending setup slot {raw} outside the cart index space");
            }
            neutral
        })
        .collect()
}

fn listing(i: net::VendingShopItem) -> VendingListing {
    VendingListing {
        index: i.index,
        nameid: i.nameid,
        type_: i.r#type,
        amount: i.amount,
        identified: i.identified,
        attribute: i.attribute,
        refine: i.refine,
        cards: i.cards,
        options: item_options(i.options),
        price: i.price,
        signer_name: i.signer_name,
        creator_id: i.creator_id,
        creator_kind: creator_kind(i.creator_kind),
        enchant_grade: i.enchant_grade,
    }
}

fn listings(items: Vec<net::VendingShopItem>) -> Vec<VendingListing> {
    items.into_iter().map(listing).collect()
}

fn creator_kind(value: i32) -> VendingCreatorKind {
    match net::CreatorKind::try_from(value) {
        Ok(net::CreatorKind::CreatorSigned) => VendingCreatorKind::Signed,
        Ok(net::CreatorKind::CreatorForged) => VendingCreatorKind::Forged,
        Ok(_) => VendingCreatorKind::None,
        Err(_) => VendingCreatorKind::Unknown(value),
    }
}

fn open_outcome(code: i32) -> VendingOpenOutcome {
    match net::VendingOpenResultCode::try_from(code) {
        Ok(net::VendingOpenResultCode::VendOk) => VendingOpenOutcome::Success,
        Ok(net::VendingOpenResultCode::VendNoCart) => VendingOpenOutcome::NoCart,
        Ok(net::VendingOpenResultCode::VendSkillNotLearned) => VendingOpenOutcome::SkillNotLearned,
        Ok(net::VendingOpenResultCode::VendTooManySlots) => VendingOpenOutcome::TooManySlots,
        Ok(net::VendingOpenResultCode::VendInvalidAmount) => VendingOpenOutcome::InvalidAmount,
        Ok(net::VendingOpenResultCode::VendInvalidPrice) => VendingOpenOutcome::InvalidPrice,
        Ok(net::VendingOpenResultCode::VendItemNotInCart) => VendingOpenOutcome::ItemNotInCart,
        Ok(net::VendingOpenResultCode::VendInsufficientStock) => {
            VendingOpenOutcome::InsufficientStock
        }
        Ok(net::VendingOpenResultCode::VendInvalidState) => VendingOpenOutcome::InvalidState,
        Ok(net::VendingOpenResultCode::VendInsufficientSp) => VendingOpenOutcome::InsufficientSp,
        Ok(net::VendingOpenResultCode::VendNpcNearby) => VendingOpenOutcome::NpcNearby,
        Ok(net::VendingOpenResultCode::VendMapRestricted) => VendingOpenOutcome::MapRestricted,
        Ok(net::VendingOpenResultCode::VendCellRestricted) => VendingOpenOutcome::CellRestricted,
        Ok(net::VendingOpenResultCode::VendInvalidTitle) => VendingOpenOutcome::InvalidTitle,
        Ok(net::VendingOpenResultCode::VendNoItems) => VendingOpenOutcome::NoItems,
        Ok(net::VendingOpenResultCode::VendDuplicateSlot) => VendingOpenOutcome::DuplicateSlot,
        Ok(net::VendingOpenResultCode::VendItemUnidentified) => {
            VendingOpenOutcome::ItemUnidentified
        }
        Ok(net::VendingOpenResultCode::VendItemBroken) => VendingOpenOutcome::ItemBroken,
        Ok(net::VendingOpenResultCode::VendItemRental) => VendingOpenOutcome::ItemRental,
        Ok(net::VendingOpenResultCode::VendItemBound) => VendingOpenOutcome::ItemBound,
        Ok(net::VendingOpenResultCode::VendItemUntradeable) => VendingOpenOutcome::ItemUntradeable,
        Ok(net::VendingOpenResultCode::VendSetupExpired) => VendingOpenOutcome::SetupExpired,
        Ok(net::VendingOpenResultCode::VendForbidden) => VendingOpenOutcome::Forbidden,
        Ok(net::VendingOpenResultCode::VendZenyLimit) => VendingOpenOutcome::ZenyLimit,
        Ok(net::VendingOpenResultCode::VendPersistenceFailed) => {
            VendingOpenOutcome::PersistenceFailed
        }
        Ok(net::VendingOpenResultCode::VendInvalidRequest) => VendingOpenOutcome::InvalidRequest,
        Err(_) => VendingOpenOutcome::Unknown(code),
    }
}

fn list_outcome(code: i32) -> VendingListOutcome {
    match net::VendingListResultCode::try_from(code) {
        Ok(net::VendingListResultCode::VendListOk) => VendingListOutcome::Success,
        Ok(net::VendingListResultCode::VendListNotOpen) => VendingListOutcome::NotOpen,
        Ok(net::VendingListResultCode::VendListShopChanged) => VendingListOutcome::ShopChanged,
        Ok(net::VendingListResultCode::VendListForbidden) => VendingListOutcome::Forbidden,
        Ok(net::VendingListResultCode::VendListInvalidRequest) => {
            VendingListOutcome::InvalidRequest
        }
        Ok(net::VendingListResultCode::VendListInternalError) => VendingListOutcome::InternalError,
        Ok(net::VendingListResultCode::VendListUnspecified) | Err(_) => {
            VendingListOutcome::Unknown(code)
        }
    }
}

fn purchase_outcome(code: i32) -> VendingPurchaseOutcome {
    match net::VendingPurchaseResultCode::try_from(code) {
        Ok(net::VendingPurchaseResultCode::VendPurchaseOk) => VendingPurchaseOutcome::Success,
        Ok(net::VendingPurchaseResultCode::VendPurchaseNotOpen) => VendingPurchaseOutcome::NotOpen,
        Ok(net::VendingPurchaseResultCode::VendPurchaseShopChanged) => {
            VendingPurchaseOutcome::ShopChanged
        }
        Ok(net::VendingPurchaseResultCode::VendPurchasePriceChanged) => {
            VendingPurchaseOutcome::PriceChanged
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseInsufficientStock) => {
            VendingPurchaseOutcome::InsufficientStock
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseInvalidLine) => {
            VendingPurchaseOutcome::InvalidLine
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseInvalidAmount) => {
            VendingPurchaseOutcome::InvalidAmount
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseNotEnoughZeny) => {
            VendingPurchaseOutcome::NotEnoughZeny
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseOverweight) => {
            VendingPurchaseOutcome::Overweight
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseNoSlots) => VendingPurchaseOutcome::NoSlots,
        Ok(net::VendingPurchaseResultCode::VendPurchaseStackLimit) => {
            VendingPurchaseOutcome::StackLimit
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseSellerZenyLimit) => {
            VendingPurchaseOutcome::SellerZenyLimit
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseForbidden) => {
            VendingPurchaseOutcome::Forbidden
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseOutOfRange) => {
            VendingPurchaseOutcome::OutOfRange
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseWrongMap) => {
            VendingPurchaseOutcome::WrongMap
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseSelfPurchase) => {
            VendingPurchaseOutcome::SelfPurchase
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseInvalidRequest) => {
            VendingPurchaseOutcome::InvalidRequest
        }
        Ok(net::VendingPurchaseResultCode::VendPurchasePersistenceFailed) => {
            VendingPurchaseOutcome::PersistenceFailed
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseInternalError) => {
            VendingPurchaseOutcome::InternalError
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseStaleRequest) => {
            VendingPurchaseOutcome::StaleRequest
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseInvalidState) => {
            VendingPurchaseOutcome::InvalidState
        }
        Ok(net::VendingPurchaseResultCode::VendPurchaseUnspecified) | Err(_) => {
            VendingPurchaseOutcome::Unknown(code)
        }
    }
}

fn close_outcome(code: i32) -> VendingCloseOutcome {
    match net::VendingCloseResultCode::try_from(code) {
        Ok(net::VendingCloseResultCode::VendCloseShopClosed) => VendingCloseOutcome::ShopClosed,
        Ok(net::VendingCloseResultCode::VendCloseSetupCanceled) => {
            VendingCloseOutcome::SetupCanceled
        }
        Ok(net::VendingCloseResultCode::VendCloseAlreadyClosed) => {
            VendingCloseOutcome::AlreadyClosed
        }
        Ok(net::VendingCloseResultCode::VendCloseInvalidTarget) => {
            VendingCloseOutcome::InvalidTarget
        }
        Ok(net::VendingCloseResultCode::VendCloseForbidden) => VendingCloseOutcome::Forbidden,
        Ok(net::VendingCloseResultCode::VendCloseInvalidRequest) => {
            VendingCloseOutcome::InvalidRequest
        }
        Ok(net::VendingCloseResultCode::VendCloseInternalError) => {
            VendingCloseOutcome::InternalError
        }
        Ok(net::VendingCloseResultCode::VendCloseUnspecified) | Err(_) => {
            VendingCloseOutcome::Unknown(code)
        }
    }
}

fn closure_reason(code: i32) -> VendingClosureReason {
    match net::VendingCloseReason::try_from(code) {
        Ok(net::VendingCloseReason::VendClosureUserClosed) => VendingClosureReason::UserClosed,
        Ok(net::VendingCloseReason::VendClosureSoldOut) => VendingClosureReason::SoldOut,
        Ok(net::VendingCloseReason::VendClosureDisconnected) => VendingClosureReason::Disconnected,
        Ok(net::VendingCloseReason::VendClosureJobChanged) => VendingClosureReason::JobChanged,
        Ok(net::VendingCloseReason::VendClosureRelocated) => VendingClosureReason::Relocated,
        Ok(net::VendingCloseReason::VendClosureInvalidated) => VendingClosureReason::Invalidated,
        Ok(net::VendingCloseReason::VendClosureUnspecified) | Err(_) => {
            VendingClosureReason::Unknown(code)
        }
    }
}

fn close_result_target(
    target: Option<net::vending_close_result::Target>,
) -> Option<VendingCloseTarget> {
    match target {
        Some(net::vending_close_result::Target::SetupId(id)) => Some(VendingCloseTarget::Setup(id)),
        Some(net::vending_close_result::Target::ShopId(id)) => Some(VendingCloseTarget::Shop(id)),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::ItemOption;

    fn shop_item() -> net::VendingShopItem {
        net::VendingShopItem {
            index: 3,
            nameid: 1201,
            r#type: 5,
            amount: 2,
            identified: false,
            attribute: 1,
            refine: 7,
            cards: vec![4001, 4002],
            price: 1_000,
            signer_name: "Odin".into(),
            creator_id: 42,
            creator_kind: net::CreatorKind::CreatorSigned as i32,
            options: vec![
                net::ItemOption {
                    id: 3,
                    value: -5,
                    param: 9,
                },
                net::ItemOption {
                    id: 0,
                    value: 0,
                    param: 0,
                },
            ],
            enchant_grade: 4,
        }
    }

    #[test]
    fn cart_offset_is_checked_in_both_directions() {
        assert_eq!(raw_cart_index(2), Some(0));
        assert_eq!(raw_cart_index(1), None);
        assert_eq!(neutral_cart_index(0), Some(2));
        assert_eq!(neutral_cart_index(u32::MAX), None);
    }

    #[test]
    fn setup_ready_maps_limits_and_neutral_slots() {
        let payload = vending_payload(Body::VendingSetupReady(net::VendingSetupReady {
            setup_id: "setup-1".into(),
            max_slots: 12,
            min_unit_price: 0,
            max_unit_price: 1_000_000_000,
            max_title_bytes: 79,
            sellable_cart_indices: vec![0, 3, u32::MAX],
            sp_spent: 30,
        }))
        .expect("setup payload");

        let VendingIngressPayload::SetupReady(setup) = payload else {
            panic!("expected setup payload, got {payload:?}");
        };
        assert_eq!(setup.setup_id, "setup-1");
        assert_eq!(setup.max_slots, 12);
        assert_eq!(setup.min_unit_price, 0);
        assert_eq!(setup.max_unit_price, 1_000_000_000);
        assert_eq!(setup.max_title_bytes, 79);
        assert_eq!(setup.sellable_cart_indices, vec![2, 5]);
        assert_eq!(setup.sp_spent, 30);
    }

    #[test]
    fn open_success_carries_full_instance_snapshot() {
        let payload = vending_payload(Body::VendingOpenResult(net::VendingOpenResult {
            result: net::VendingOpenResultCode::VendOk as i32,
            request_id: 8,
            setup_id: "setup-1".into(),
            setup_active: false,
            shop_id: "shop-1".into(),
            title: "Potions".into(),
            items: vec![shop_item()],
            vendor_unit_id: 77,
        }))
        .expect("open payload");

        let VendingIngressPayload::OpenResult {
            request_id,
            setup_id,
            setup_active,
            outcome,
            snapshot,
        } = payload
        else {
            panic!("expected open payload");
        };
        assert_eq!(request_id, 8);
        assert_eq!(setup_id, "setup-1");
        assert!(!setup_active);
        assert_eq!(outcome, VendingOpenOutcome::Success);
        let snapshot = snapshot.expect("successful open snapshot");
        assert_eq!(snapshot.vendor_unit_id, 77);
        assert_eq!(snapshot.shop_id, "shop-1");
        assert_eq!(snapshot.title, "Potions");
        assert_eq!(snapshot.total_earned, 0);
        assert_eq!(snapshot.items.len(), 1);
        let item = &snapshot.items[0];
        assert_eq!(item.index, 3);
        assert_eq!(item.nameid, 1201);
        assert_eq!(item.type_, 5);
        assert_eq!(item.amount, 2);
        assert!(!item.identified);
        assert_eq!(item.attribute, 1);
        assert_eq!(item.refine, 7);
        assert_eq!(item.cards, vec![4001, 4002]);
        assert_eq!(item.price, 1_000);
        assert_eq!(item.signer_name, "Odin");
        assert_eq!(item.creator_id, 42);
        assert_eq!(item.creator_kind, VendingCreatorKind::Signed);
        assert_eq!(item.enchant_grade, 4);
        assert_eq!(
            item.options,
            vec![
                ItemOption {
                    id: 3,
                    value: -5,
                    param: 9
                },
                ItemOption {
                    id: 0,
                    value: 0,
                    param: 0
                }
            ]
        );
    }

    #[test]
    fn rejected_open_has_no_snapshot_and_keeps_setup_when_active() {
        let payload = vending_payload(Body::VendingOpenResult(net::VendingOpenResult {
            result: net::VendingOpenResultCode::VendDuplicateSlot as i32,
            request_id: 9,
            setup_id: "setup-1".into(),
            setup_active: true,
            shop_id: "stale".into(),
            title: "stale".into(),
            items: vec![shop_item()],
            vendor_unit_id: 77,
        }))
        .expect("open payload");

        let VendingIngressPayload::OpenResult {
            setup_active,
            outcome,
            snapshot,
            ..
        } = payload
        else {
            panic!("expected open payload");
        };
        assert!(setup_active);
        assert_eq!(outcome, VendingOpenOutcome::DuplicateSlot);
        assert!(snapshot.is_none());
    }

    #[test]
    fn unknown_and_unspecified_codes_are_preserved_not_successes() {
        let payload = vending_payload(Body::VendingOpenResult(net::VendingOpenResult {
            result: 999,
            request_id: 1,
            setup_id: String::new(),
            setup_active: false,
            shop_id: String::new(),
            title: String::new(),
            items: vec![],
            vendor_unit_id: 1,
        }))
        .expect("open payload");
        assert!(matches!(
            payload,
            VendingIngressPayload::OpenResult {
                outcome: VendingOpenOutcome::Unknown(999),
                snapshot: None,
                ..
            }
        ));

        for body in [
            Body::VendingList(net::VendingList {
                vendor_unit_id: 1,
                title: String::new(),
                items: vec![],
                request_id: 2,
                shop_id: String::new(),
                result: 0,
                total_earned: 0,
            }),
            Body::VendingPurchaseResult(net::VendingPurchaseResult {
                request_id: 3,
                vendor_unit_id: 1,
                shop_id: String::new(),
                result: 0,
                total_charged: 0,
            }),
            Body::VendingCloseResult(net::VendingCloseResult {
                request_id: 4,
                result: 0,
                target: Some(net::vending_close_result::Target::ShopId("s".into())),
            }),
        ] {
            let payload = vending_payload(body).expect("payload");
            let unknown = match payload {
                VendingIngressPayload::ListResult {
                    outcome, snapshot, ..
                } => {
                    assert!(snapshot.is_none());
                    outcome == VendingListOutcome::Unknown(0)
                }
                VendingIngressPayload::PurchaseResult { outcome, .. } => {
                    outcome == VendingPurchaseOutcome::Unknown(0)
                }
                VendingIngressPayload::CloseResult { outcome, .. } => {
                    outcome == VendingCloseOutcome::Unknown(0)
                }
                other => panic!("unexpected payload {other:?}"),
            };
            assert!(unknown);
        }
    }

    #[test]
    fn owner_list_snapshot_carries_identity_and_net_earnings() {
        let payload = vending_payload(Body::VendingList(net::VendingList {
            vendor_unit_id: 5,
            title: "Scrolls".into(),
            items: vec![shop_item()],
            request_id: 0,
            shop_id: "shop-2".into(),
            result: net::VendingListResultCode::VendListOk as i32,
            total_earned: 12_345,
        }))
        .expect("list payload");

        let VendingIngressPayload::ListResult {
            request_id,
            vendor_unit_id,
            outcome,
            snapshot,
        } = payload
        else {
            panic!("expected list payload");
        };
        assert_eq!(request_id, 0);
        assert_eq!(vendor_unit_id, 5);
        assert_eq!(outcome, VendingListOutcome::Success);
        let snapshot = snapshot.expect("list snapshot");
        assert_eq!(snapshot.vendor_unit_id, 5);
        assert_eq!(snapshot.shop_id, "shop-2");
        assert_eq!(snapshot.title, "Scrolls");
        assert_eq!(snapshot.total_earned, 12_345);
        assert_eq!(snapshot.items.len(), 1);
    }

    #[test]
    fn purchase_result_keeps_identity_and_charge() {
        let payload = vending_payload(Body::VendingPurchaseResult(net::VendingPurchaseResult {
            request_id: 11,
            vendor_unit_id: 5,
            shop_id: "shop-2".into(),
            result: net::VendingPurchaseResultCode::VendPurchasePriceChanged as i32,
            total_charged: 0,
        }))
        .expect("purchase payload");
        assert_eq!(
            payload,
            VendingIngressPayload::PurchaseResult {
                request_id: 11,
                vendor_unit_id: 5,
                shop_id: "shop-2".into(),
                outcome: VendingPurchaseOutcome::PriceChanged,
                total_charged: 0,
            }
        );
    }

    #[test]
    fn close_result_echoes_attempted_target() {
        let payload = vending_payload(Body::VendingCloseResult(net::VendingCloseResult {
            request_id: 12,
            result: net::VendingCloseResultCode::VendCloseSetupCanceled as i32,
            target: Some(net::vending_close_result::Target::SetupId("setup-9".into())),
        }))
        .expect("close payload");
        assert_eq!(
            payload,
            VendingIngressPayload::CloseResult {
                request_id: 12,
                target: VendingCloseTarget::Setup("setup-9".into()),
                outcome: VendingCloseOutcome::SetupCanceled,
            }
        );

        assert!(
            vending_payload(Body::VendingCloseResult(net::VendingCloseResult {
                request_id: 12,
                result: 0,
                target: None,
            }))
            .is_none()
        );
    }

    #[test]
    fn boards_reports_and_invalidations_are_ordered_payloads() {
        let shown = vending_payload(Body::VendingBoardShown(net::VendingBoardShown {
            unit_id: 5,
            title: "Potions".into(),
            shop_id: "shop-2".into(),
        }))
        .expect("board payload");
        assert_eq!(
            shown,
            VendingIngressPayload::BoardShown(VendingBoard {
                vendor_unit_id: 5,
                shop_id: "shop-2".into(),
                title: "Potions".into(),
            })
        );

        let removed = vending_payload(Body::VendingBoardRemoved(net::VendingBoardRemoved {
            unit_id: 5,
            shop_id: "shop-2".into(),
            reason: net::VendingCloseReason::VendClosureSoldOut as i32,
        }))
        .expect("board removal payload");
        assert_eq!(
            removed,
            VendingIngressPayload::BoardRemoved {
                unit_id: 5,
                shop_id: "shop-2".into(),
                reason: VendingClosureReason::SoldOut,
            }
        );

        let report = vending_payload(Body::VendingSaleReport(net::VendingSaleReport {
            index: 3,
            nameid: 501,
            amount: 2,
            zeny_gained: 500,
            buyer_name: "Thor".into(),
            shop_id: "shop-2".into(),
            sale_sequence: 4,
            total_earned: 900,
        }))
        .expect("sale report payload");
        assert_eq!(
            report,
            VendingIngressPayload::SaleReport {
                shop_id: "shop-2".into(),
                index: 3,
                nameid: 501,
                amount: 2,
                zeny_gained: 500,
                buyer_name: "Thor".into(),
                sale_sequence: 4,
                total_earned: 900,
            }
        );

        assert_eq!(
            vending_payload(Body::UnitDespawn(net::UnitDespawn { gid: 5, reason: 0 })),
            Some(VendingIngressPayload::VendorDespawned { unit_id: 5 })
        );
        assert_eq!(
            vending_payload(Body::MapMove(net::MapMove {
                map_name: "prontera".into(),
                x: 1,
                y: 2,
            })),
            Some(VendingIngressPayload::MapChanging)
        );
    }

    #[test]
    fn unrelated_bodies_are_ignored() {
        assert!(
            vending_payload(Body::CartMountResult(net::CartMountResult { result: 0 })).is_none()
        );
        assert!(
            vending_payload(Body::VendingOpenRequest(net::VendingOpenRequest {
                title: "outbound".into(),
                entries: vec![],
                request_id: 1,
                setup_id: "s".into(),
            }))
            .is_none()
        );
    }
}
