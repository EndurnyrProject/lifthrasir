//! Authoritative client-side vending state, the intents that request changes,
//! and the neutral notices that settle them.

use std::collections::HashMap;

use bevy::prelude::*;
use net_contract::dto::{
    VendingBoard, VendingBuyLine, VendingCloseOutcome, VendingCloseTarget, VendingClosureReason,
    VendingListOutcome, VendingOffer, VendingOpenOutcome, VendingPurchaseOutcome, VendingSetup,
    VendingSnapshot,
};
use net_contract::events::{VendingIngress, VendingIngressPayload};
use net_contract::state::ZoneSessionGeneration;

/// How long a mutation may stay unanswered before it is shown as unconfirmed.
pub const MUTATION_TIMEOUT_SECS: f32 = 10.0;

/// Sales kept for the seller monitor.
pub const RECENT_SALES: usize = 8;

/// Written when the seller window should be revealed without casting again.
#[derive(Message, Debug, Clone, Copy)]
pub struct RevealVendingSeller;

/// A local request to change, inspect, or buy from a vendor.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub enum VendingIntent {
    EnterSeller,
    Browse {
        vendor_unit_id: u64,
        shop_id: String,
    },
    Open {
        setup_id: String,
        title: String,
        entries: Vec<VendingOffer>,
    },
    Close {
        target: VendingCloseTarget,
    },
    Purchase {
        vendor_unit_id: u64,
        shop_id: String,
        items: Vec<VendingBuyLine>,
    },
    Refresh,
}

/// Neutral settlement of a request, for UI feedback. Carries no UI text.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub enum VendingNotice {
    Open {
        request_id: u64,
        setup_id: String,
        outcome: VendingOpenOutcome,
    },
    List {
        request_id: u64,
        vendor_unit_id: u64,
        outcome: VendingListOutcome,
    },
    Purchase {
        request_id: u64,
        vendor_unit_id: u64,
        shop_id: String,
        outcome: VendingPurchaseOutcome,
    },
    Close {
        request_id: u64,
        target: VendingCloseTarget,
        outcome: VendingCloseOutcome,
    },
    Unconfirmed {
        request_id: u64,
    },
}

/// One reported sale, kept for the seller monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendingSale {
    pub shop_id: String,
    pub index: u32,
    pub nameid: u32,
    pub amount: u32,
    pub zeny_gained: u32,
    pub buyer_name: String,
    pub sale_sequence: u64,
}

/// A read request for one vendor, with the snapshot it last produced.
#[derive(Debug, Clone)]
struct Browse {
    request_id: u64,
    vendor_unit_id: u64,
    snapshot: Option<VendingSnapshot>,
}

#[derive(Debug, Clone, Copy)]
struct PendingOpen {
    request_id: u64,
    since: f32,
    stale: bool,
}

#[derive(Debug, Clone)]
struct PendingClose {
    request_id: u64,
    target: VendingCloseTarget,
    since: f32,
    stale: bool,
}

#[derive(Debug, Clone)]
struct PendingPurchase {
    request_id: u64,
    vendor_unit_id: u64,
    shop_id: String,
    since: f32,
    stale: bool,
}

/// Requests a reload of one shop; produced by a successful checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendingRefresh {
    pub request_id: u64,
    pub vendor_unit_id: u64,
    pub expected_shop_id: String,
}

/// Setup, shop, browse, board, and pending-request state for online vending.
#[derive(Resource, Debug, Default)]
pub struct VendingState {
    setup: Option<VendingSetup>,
    opening: Option<PendingOpen>,
    active: Option<VendingSnapshot>,
    last_closed: Option<VendingSnapshot>,
    browse: Option<Browse>,
    closing: Option<PendingClose>,
    purchase: Option<PendingPurchase>,
    boards: HashMap<u64, VendingBoard>,
    sales: Vec<VendingSale>,
    sale_watermarks: HashMap<String, u64>,
    notices: Vec<VendingNotice>,
    refresh: Option<VendingRefresh>,
    next_request_id: u64,
    owner_unconfirmed: bool,
    session_generation: ZoneSessionGeneration,
    session_char_id: u32,
}

impl VendingState {
    /// Applies one ordered authoritative payload.
    pub fn apply(&mut self, incoming: &VendingIngress) {
        match &incoming.payload {
            VendingIngressPayload::SetupReady(setup) => self.apply_setup_ready(setup),
            VendingIngressPayload::OpenResult {
                request_id,
                setup_id,
                setup_active,
                outcome,
                snapshot,
            } => self.apply_open_result(*request_id, setup_id, *setup_active, *outcome, snapshot),
            VendingIngressPayload::ListResult {
                request_id,
                vendor_unit_id,
                outcome,
                snapshot,
            } => self.apply_list_result(*request_id, *vendor_unit_id, *outcome, snapshot),
            VendingIngressPayload::PurchaseResult {
                request_id,
                vendor_unit_id,
                shop_id,
                outcome,
                total_charged: _,
            } => self.apply_purchase_result(*request_id, *vendor_unit_id, shop_id, *outcome),
            VendingIngressPayload::CloseResult {
                request_id,
                target,
                outcome,
            } => self.apply_close_result(*request_id, target, *outcome),
            VendingIngressPayload::BoardShown(board) => {
                self.boards.insert(board.vendor_unit_id, board.clone());
            }
            VendingIngressPayload::BoardRemoved {
                unit_id,
                shop_id,
                reason,
            } => self.apply_board_removed(*unit_id, shop_id, *reason),
            VendingIngressPayload::SaleReport {
                shop_id,
                index,
                nameid,
                amount,
                zeny_gained,
                buyer_name,
                sale_sequence,
                total_earned,
            } => self.apply_sale(
                &VendingSale {
                    shop_id: shop_id.clone(),
                    index: *index,
                    nameid: *nameid,
                    amount: *amount,
                    zeny_gained: *zeny_gained,
                    buyer_name: buyer_name.clone(),
                    sale_sequence: *sale_sequence,
                },
                *total_earned,
            ),
            VendingIngressPayload::VendorDespawned { unit_id } => {
                self.boards.remove(unit_id);
                if self
                    .browse
                    .as_ref()
                    .is_some_and(|browse| browse.vendor_unit_id == *unit_id)
                {
                    self.browse = None;
                }
            }
            VendingIngressPayload::MapChanging => self.apply_map_changing(),
        }
    }

    fn apply_setup_ready(&mut self, setup: &VendingSetup) {
        self.opening = None;
        self.setup = Some(setup.clone());
    }

    fn apply_open_result(
        &mut self,
        request_id: u64,
        setup_id: &str,
        setup_active: bool,
        outcome: VendingOpenOutcome,
        snapshot: &Option<VendingSnapshot>,
    ) {
        if !self
            .opening
            .as_ref()
            .is_some_and(|pending| pending.request_id == request_id)
        {
            warn!("ignoring late vending open result {request_id}");
            return;
        }
        self.opening = None;

        match (outcome, snapshot) {
            (VendingOpenOutcome::Success, Some(snapshot)) => {
                self.setup = None;
                self.active = Some(snapshot.clone());
                self.owner_unconfirmed = false;
            }
            (VendingOpenOutcome::Success, None) => {
                warn!("vending open succeeded without a shop snapshot");
                self.setup = None;
            }
            (_, _) if !setup_active => self.setup = None,
            (_, _)
                if !self
                    .setup
                    .as_ref()
                    .is_some_and(|setup| setup.setup_id == setup_id) =>
            {
                self.setup = None;
            }
            _ => {}
        }

        self.notices.push(VendingNotice::Open {
            request_id,
            setup_id: setup_id.to_string(),
            outcome,
        });
    }

    fn apply_list_result(
        &mut self,
        request_id: u64,
        vendor_unit_id: u64,
        outcome: VendingListOutcome,
        snapshot: &Option<VendingSnapshot>,
    ) {
        if request_id == 0 {
            self.apply_owner_snapshot(vendor_unit_id, outcome, snapshot);
            return;
        }

        let Some(browse) = self.browse.as_mut() else {
            warn!("ignoring vending list result {request_id} without a browse request");
            return;
        };
        if browse.request_id != request_id || browse.vendor_unit_id != vendor_unit_id {
            warn!("ignoring superseded vending list result {request_id}");
            return;
        }
        browse.snapshot = match (outcome, snapshot) {
            (VendingListOutcome::Success, Some(snapshot)) => Some(snapshot.clone()),
            _ => None,
        };
        self.notices.push(VendingNotice::List {
            request_id,
            vendor_unit_id,
            outcome,
        });
    }

    /// Unsolicited own-shop reconciliation after a map load.
    fn apply_owner_snapshot(
        &mut self,
        vendor_unit_id: u64,
        outcome: VendingListOutcome,
        snapshot: &Option<VendingSnapshot>,
    ) {
        self.owner_unconfirmed = false;
        match (outcome, snapshot) {
            (VendingListOutcome::Success, Some(snapshot))
                if snapshot.vendor_unit_id == vendor_unit_id =>
            {
                self.active = Some(snapshot.clone());
            }
            (VendingListOutcome::Success, Some(_)) => {
                warn!("ignoring own vending snapshot for a different vendor");
            }
            _ => {
                self.active = None;
            }
        }
    }

    fn apply_purchase_result(
        &mut self,
        request_id: u64,
        vendor_unit_id: u64,
        shop_id: &str,
        outcome: VendingPurchaseOutcome,
    ) {
        let matches_pending = self.purchase.as_ref().is_some_and(|pending| {
            pending.request_id == request_id
                && pending.vendor_unit_id == vendor_unit_id
                && pending.shop_id == shop_id
        });
        if !matches_pending {
            warn!("ignoring unmatched vending purchase result {request_id}");
            return;
        }
        self.purchase = None;

        if matches!(outcome, VendingPurchaseOutcome::Success) {
            let refresh = self.queue_refresh(vendor_unit_id, shop_id);
            self.refresh = Some(refresh);
        }

        self.notices.push(VendingNotice::Purchase {
            request_id,
            vendor_unit_id,
            shop_id: shop_id.to_string(),
            outcome,
        });
    }

    fn apply_close_result(
        &mut self,
        request_id: u64,
        target: &VendingCloseTarget,
        outcome: VendingCloseOutcome,
    ) {
        let matches_pending = self
            .closing
            .as_ref()
            .is_some_and(|pending| pending.request_id == request_id && pending.target == *target);
        if !matches_pending {
            warn!("ignoring unmatched vending close result {request_id}");
            return;
        }
        self.closing = None;

        match outcome {
            VendingCloseOutcome::SetupCanceled
                if self
                    .setup
                    .as_ref()
                    .is_some_and(|setup| matches!(target, VendingCloseTarget::Setup(id) if *id == setup.setup_id)) =>
            {
                self.setup = None;
            }
            VendingCloseOutcome::ShopClosed | VendingCloseOutcome::AlreadyClosed
                if self
                    .active
                    .as_ref()
                    .is_some_and(|shop| matches!(target, VendingCloseTarget::Shop(id) if *id == shop.shop_id)) =>
            {
                self.last_closed = self.active.take();
            }
            _ => {}
        }

        self.notices.push(VendingNotice::Close {
            request_id,
            target: target.clone(),
            outcome,
        });
    }

    fn apply_board_removed(&mut self, unit_id: u64, shop_id: &str, reason: VendingClosureReason) {
        if self
            .boards
            .get(&unit_id)
            .is_some_and(|board| board.shop_id == shop_id)
        {
            self.boards.remove(&unit_id);
        }
        if self.browse.as_ref().is_some_and(|browse| {
            browse.vendor_unit_id == unit_id
                && browse
                    .snapshot
                    .as_ref()
                    .is_some_and(|s| s.shop_id == shop_id)
        }) {
            self.browse = None;
        }
        if self
            .active
            .as_ref()
            .is_some_and(|shop| shop.vendor_unit_id == unit_id && shop.shop_id == shop_id)
            && matches!(
                reason,
                VendingClosureReason::UserClosed | VendingClosureReason::SoldOut
            )
        {
            self.last_closed = self.active.take();
        }
    }

    fn apply_sale(&mut self, sale: &VendingSale, total_earned: u64) {
        let owns_shop = self
            .active
            .as_ref()
            .is_some_and(|shop| shop.shop_id == sale.shop_id)
            || self
                .last_closed
                .as_ref()
                .is_some_and(|shop| shop.shop_id == sale.shop_id);
        if !owns_shop {
            warn!("ignoring vending sale report for shop {}", sale.shop_id);
            return;
        }
        let watermark = self
            .sale_watermarks
            .get(&sale.shop_id)
            .copied()
            .unwrap_or(0);
        if sale.sale_sequence <= watermark {
            warn!("ignoring replayed vending sale {}", sale.sale_sequence);
            return;
        }
        self.sale_watermarks
            .insert(sale.shop_id.clone(), sale.sale_sequence);

        if let Some(shop) = self
            .active
            .as_mut()
            .filter(|shop| shop.shop_id == sale.shop_id)
            .or_else(|| {
                self.last_closed
                    .as_mut()
                    .filter(|shop| shop.shop_id == sale.shop_id)
            })
        {
            shop.total_earned = total_earned;
        }
        self.sales.push(sale.clone());
        if self.sales.len() > RECENT_SALES {
            self.sales.remove(0);
        }
    }

    fn apply_map_changing(&mut self) {
        self.browse = None;
        self.boards.clear();
        self.owner_unconfirmed = true;
    }

    /// Clears every session-owned value, including request identity.
    pub fn reset_session(&mut self) {
        *self = Self {
            session_generation: self.session_generation,
            session_char_id: self.session_char_id,
            ..Self::default()
        };
    }

    pub fn cart_transfers_locked(&self) -> bool {
        self.setup.is_some() || self.active.is_some()
    }

    pub fn gameplay_locked(&self) -> bool {
        self.active.is_some() || self.owner_unconfirmed
    }

    pub fn active_shop(&self) -> Option<&VendingSnapshot> {
        self.active.as_ref()
    }

    pub fn setup(&self) -> Option<&VendingSetup> {
        self.setup.as_ref()
    }

    pub fn browse(&self) -> Option<&VendingSnapshot> {
        self.browse
            .as_ref()
            .and_then(|browse| browse.snapshot.as_ref())
    }

    pub fn last_closed_shop(&self) -> Option<&VendingSnapshot> {
        self.last_closed.as_ref()
    }

    pub fn boards(&self) -> &HashMap<u64, VendingBoard> {
        &self.boards
    }

    pub fn recent_sales(&self) -> &[VendingSale] {
        &self.sales
    }

    pub fn purchase_pending(&self) -> bool {
        self.purchase.is_some()
    }

    pub fn open_pending(&self) -> bool {
        self.opening.is_some()
    }

    pub fn close_pending(&self) -> bool {
        self.closing.is_some()
    }

    pub fn owner_authority_unconfirmed(&self) -> bool {
        self.owner_unconfirmed
    }

    pub fn browse_pending(&self) -> bool {
        self.browse
            .as_ref()
            .is_some_and(|browse| browse.snapshot.is_none())
    }

    /// Issues one open request against a live setup grant.
    pub fn begin_open(&mut self, setup_id: &str, now: f32) -> Option<u64> {
        if self.opening.is_some()
            || !self
                .setup
                .as_ref()
                .is_some_and(|setup| setup.setup_id == setup_id)
        {
            return None;
        }
        let request_id = self.allocate();
        self.opening = Some(PendingOpen {
            request_id,
            since: now,
            stale: false,
        });
        Some(request_id)
    }

    /// Issues one close request; a second one waits for the first.
    pub fn begin_close(&mut self, target: &VendingCloseTarget, now: f32) -> Option<u64> {
        if self.closing.is_some() {
            return None;
        }
        let request_id = self.allocate();
        self.closing = Some(PendingClose {
            request_id,
            target: target.clone(),
            since: now,
            stale: false,
        });
        Some(request_id)
    }

    /// Issues one checkout; no second checkout is accepted until it settles.
    pub fn begin_purchase(&mut self, vendor_unit_id: u64, shop_id: &str, now: f32) -> Option<u64> {
        if self.purchase.is_some() {
            return None;
        }
        let request_id = self.allocate();
        self.purchase = Some(PendingPurchase {
            request_id,
            vendor_unit_id,
            shop_id: shop_id.to_string(),
            since: now,
            stale: false,
        });
        Some(request_id)
    }

    /// Starts browsing one vendor, or supersedes a request for another one.
    pub fn begin_browse(&mut self, vendor_unit_id: u64) -> Option<u64> {
        if self.browse.as_ref().is_some_and(|browse| {
            browse.vendor_unit_id == vendor_unit_id && browse.snapshot.is_none()
        }) {
            return None;
        }
        let request_id = self.allocate();
        let previous = self
            .browse
            .take()
            .filter(|previous| previous.vendor_unit_id == vendor_unit_id)
            .and_then(|previous| previous.snapshot);
        self.browse = Some(Browse {
            request_id,
            vendor_unit_id,
            snapshot: previous,
        });
        Some(request_id)
    }

    /// Reloads the shop currently shown or owned.
    pub fn begin_refresh(&mut self) -> Option<VendingRefresh> {
        let (vendor_unit_id, shop_id) = self
            .browse
            .as_ref()
            .and_then(|browse| {
                browse
                    .snapshot
                    .as_ref()
                    .map(|snapshot| (browse.vendor_unit_id, snapshot.shop_id.clone()))
            })
            .or_else(|| {
                self.active
                    .as_ref()
                    .map(|shop| (shop.vendor_unit_id, shop.shop_id.clone()))
            })?;
        Some(self.queue_refresh(vendor_unit_id, &shop_id))
    }

    fn queue_refresh(&mut self, vendor_unit_id: u64, shop_id: &str) -> VendingRefresh {
        let request_id = self.allocate();
        let snapshot = self
            .browse
            .as_ref()
            .and_then(|browse| browse.snapshot.clone())
            .or_else(|| {
                self.active
                    .as_ref()
                    .filter(|shop| shop.shop_id == shop_id)
                    .cloned()
            });
        self.browse = Some(Browse {
            request_id,
            vendor_unit_id,
            snapshot,
        });
        VendingRefresh {
            request_id,
            vendor_unit_id,
            expected_shop_id: shop_id.to_string(),
        }
    }

    /// Read-only request queued by a settled checkout.
    pub fn take_refresh(&mut self) -> Option<VendingRefresh> {
        self.refresh.take()
    }

    pub fn take_notices(&mut self) -> Vec<VendingNotice> {
        std::mem::take(&mut self.notices)
    }

    /// Marks unanswered mutations visible without retrying or unlocking them.
    pub fn mark_unconfirmed(&mut self, now: f32) {
        let mut notices = std::mem::take(&mut self.notices);
        if let Some(pending) = self.opening.as_mut() {
            mark(
                &mut notices,
                pending.request_id,
                pending.since,
                &mut pending.stale,
                now,
            );
        }
        if let Some(pending) = self.closing.as_mut() {
            mark(
                &mut notices,
                pending.request_id,
                pending.since,
                &mut pending.stale,
                now,
            );
        }
        if let Some(pending) = self.purchase.as_mut() {
            mark(
                &mut notices,
                pending.request_id,
                pending.since,
                &mut pending.stale,
                now,
            );
        }
        self.notices = notices;
    }

    fn allocate(&mut self) -> u64 {
        self.next_request_id += 1;
        self.next_request_id
    }

    pub(crate) fn session_gate(&self) -> (ZoneSessionGeneration, u32) {
        (self.session_generation, self.session_char_id)
    }

    pub(crate) fn set_session_gate(&mut self, generation: ZoneSessionGeneration, char_id: u32) {
        self.session_generation = generation;
        self.session_char_id = char_id;
    }
}

fn mark(notices: &mut Vec<VendingNotice>, request_id: u64, since: f32, stale: &mut bool, now: f32) {
    if !*stale && now - since >= MUTATION_TIMEOUT_SECS {
        *stale = true;
        notices.push(VendingNotice::Unconfirmed { request_id });
    }
}
