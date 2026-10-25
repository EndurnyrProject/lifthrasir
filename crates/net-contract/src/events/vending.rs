//! Ordered, zone-session-scoped vending ingress from the active adapter.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

use crate::{
    dto::{
        VendingBoard, VendingCloseOutcome, VendingCloseTarget, VendingClosureReason,
        VendingListOutcome, VendingOpenOutcome, VendingPurchaseOutcome, VendingSetup,
        VendingSnapshot,
    },
    state::ZoneSessionGeneration,
};

/// One vending or visibility payload stamped with the receiving zone session.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct VendingIngress {
    pub generation: ZoneSessionGeneration,
    pub payload: VendingIngressPayload,
}

/// A single stream preserves vending and invalidation order across payload kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VendingIngressPayload {
    SetupReady(VendingSetup),
    OpenResult {
        request_id: u64,
        setup_id: String,
        setup_active: bool,
        outcome: VendingOpenOutcome,
        snapshot: Option<VendingSnapshot>,
    },
    ListResult {
        request_id: u64,
        vendor_unit_id: u64,
        outcome: VendingListOutcome,
        snapshot: Option<VendingSnapshot>,
    },
    PurchaseResult {
        request_id: u64,
        vendor_unit_id: u64,
        shop_id: String,
        outcome: VendingPurchaseOutcome,
        total_charged: u64,
    },
    CloseResult {
        request_id: u64,
        target: VendingCloseTarget,
        outcome: VendingCloseOutcome,
    },
    BoardShown(VendingBoard),
    BoardRemoved {
        unit_id: u64,
        shop_id: String,
        reason: VendingClosureReason,
    },
    SaleReport {
        shop_id: String,
        index: u32,
        nameid: u32,
        amount: u32,
        zeny_gained: u32,
        buyer_name: String,
        sale_sequence: u64,
        total_earned: u64,
    },
    VendorDespawned {
        unit_id: u64,
    },
    MapChanging,
}
