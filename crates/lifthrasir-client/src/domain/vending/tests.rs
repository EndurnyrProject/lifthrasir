//! Vending reducer, intent, and projection regressions.

use std::time::Duration;

use bevy::prelude::*;
use net_contract::commands::{
    CloseVendingRequested, ListVendingRequested, OpenVendingRequested, PurchaseVendingRequested,
};
use net_contract::dto::{
    VendingBoard, VendingBuyLine, VendingCloseOutcome, VendingCloseTarget, VendingClosureReason,
    VendingListOutcome, VendingOffer, VendingOpenOutcome, VendingPurchaseOutcome, VendingSetup,
    VendingSnapshot,
};
use net_contract::events::{VendingIngress, VendingIngressPayload, ZoneDisconnected};
use net_contract::state::ZoneSessionGeneration;

use super::plugin::VendingPlugin;
use super::state::{
    MUTATION_TIMEOUT_SECS, RevealVendingSeller, VendingIntent, VendingNotice, VendingState,
};
use super::systems::VENDING_SKILL_ID;
use crate::domain::skill::SkillCastRequested;

const SETUP_ID: &str = "setup-1";
const SHOP_ID: &str = "shop-1";
const VENDOR_ID: u64 = 5;

fn snapshot(shop_id: &str, total_earned: u64) -> VendingSnapshot {
    VendingSnapshot {
        vendor_unit_id: VENDOR_ID,
        shop_id: shop_id.into(),
        title: "Potions".into(),
        items: vec![],
        total_earned,
    }
}

fn setup(setup_id: &str) -> VendingSetup {
    VendingSetup {
        setup_id: setup_id.into(),
        max_slots: 12,
        min_unit_price: 0,
        max_unit_price: 1_000_000_000,
        max_title_bytes: 79,
        sellable_cart_indices: vec![2],
        sp_spent: 30,
    }
}

fn ready() -> VendingIngressPayload {
    VendingIngressPayload::SetupReady(setup(SETUP_ID))
}

fn open_result(
    request_id: u64,
    setup_active: bool,
    outcome: VendingOpenOutcome,
) -> VendingIngressPayload {
    VendingIngressPayload::OpenResult {
        request_id,
        setup_id: SETUP_ID.into(),
        setup_active,
        outcome,
        snapshot: matches!(outcome, VendingOpenOutcome::Success).then(|| snapshot(SHOP_ID, 0)),
    }
}

fn list_result(
    request_id: u64,
    outcome: VendingListOutcome,
    total_earned: u64,
) -> VendingIngressPayload {
    VendingIngressPayload::ListResult {
        request_id,
        vendor_unit_id: VENDOR_ID,
        outcome,
        snapshot: matches!(outcome, VendingListOutcome::Success)
            .then(|| snapshot(SHOP_ID, total_earned)),
    }
}

fn purchase_result(
    request_id: u64,
    vendor_unit_id: u64,
    shop_id: &str,
    outcome: VendingPurchaseOutcome,
) -> VendingIngressPayload {
    VendingIngressPayload::PurchaseResult {
        request_id,
        vendor_unit_id,
        shop_id: shop_id.into(),
        outcome,
        total_charged: 500,
    }
}

fn close_result(
    request_id: u64,
    target: VendingCloseTarget,
    outcome: VendingCloseOutcome,
) -> VendingIngressPayload {
    VendingIngressPayload::CloseResult {
        request_id,
        target,
        outcome,
    }
}

fn sale(sequence: u64, total_earned: u64) -> VendingIngressPayload {
    VendingIngressPayload::SaleReport {
        shop_id: SHOP_ID.into(),
        index: 2,
        nameid: 501,
        amount: 1,
        zeny_gained: 500,
        buyer_name: "Thor".into(),
        sale_sequence: sequence,
        total_earned,
    }
}

fn board() -> VendingIngressPayload {
    VendingIngressPayload::BoardShown(VendingBoard {
        vendor_unit_id: VENDOR_ID,
        shop_id: SHOP_ID.into(),
        title: "Potions".into(),
    })
}

fn ingress(payload: VendingIngressPayload) -> VendingIngress {
    VendingIngress {
        generation: ZoneSessionGeneration::default(),
        payload,
    }
}

/// Drives the seller from idle to an open shop without the UI.
fn opened_shop(state: &mut VendingState) -> u64 {
    state.apply(&ingress(ready()));
    let request_id = state.begin_open(SETUP_ID, 0.0).expect("open request");
    state.apply(&ingress(open_result(
        request_id,
        false,
        VendingOpenOutcome::Success,
    )));
    request_id
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(VendingPlugin)
        .init_resource::<Time>()
        .init_resource::<ZoneSessionGeneration>()
        .add_message::<VendingIngress>()
        .add_message::<ZoneDisconnected>()
        .add_message::<SkillCastRequested>()
        .add_message::<OpenVendingRequested>()
        .add_message::<ListVendingRequested>()
        .add_message::<PurchaseVendingRequested>()
        .add_message::<CloseVendingRequested>();
    app
}

fn write(app: &mut App, payload: VendingIngressPayload) {
    app.world_mut()
        .resource_mut::<Messages<VendingIngress>>()
        .write(ingress(payload));
}

fn notices(app: &App) -> Vec<VendingNotice> {
    app.world()
        .resource::<Messages<VendingNotice>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}

fn vending(app: &App) -> &VendingState {
    app.world().resource::<VendingState>()
}

#[test]
fn setup_grant_prepares_without_blocking_ordinary_gameplay() {
    let mut state = VendingState::default();
    state.apply(&ingress(ready()));

    assert_eq!(state.setup().map(|s| s.setup_id.as_str()), Some(SETUP_ID));
    assert!(state.active_shop().is_none());
    assert!(state.cart_transfers_locked());
    assert!(!state.gameplay_locked());
}

#[test]
fn accepted_open_becomes_the_active_monitor() {
    let mut state = VendingState::default();
    opened_shop(&mut state);

    assert!(state.setup().is_none());
    assert_eq!(
        state.active_shop().map(|shop| shop.shop_id.as_str()),
        Some(SHOP_ID)
    );
    assert!(state.cart_transfers_locked());
    assert!(state.gameplay_locked());
    assert_eq!(
        state.take_notices(),
        vec![VendingNotice::Open {
            request_id: 1,
            setup_id: SETUP_ID.into(),
            outcome: VendingOpenOutcome::Success,
        }]
    );
}

#[test]
fn rejected_open_keeps_setup_only_while_the_server_says_it_is_active() {
    let mut state = VendingState::default();
    state.apply(&ingress(ready()));
    let keep = state.begin_open(SETUP_ID, 0.0).expect("open request");
    state.apply(&ingress(open_result(
        keep,
        true,
        VendingOpenOutcome::DuplicateSlot,
    )));
    assert!(state.setup().is_some());

    let drop = state
        .begin_open(SETUP_ID, 0.0)
        .expect("second open request");
    state.apply(&ingress(open_result(
        drop,
        false,
        VendingOpenOutcome::InsufficientStock,
    )));
    assert!(state.setup().is_none());
}

#[test]
fn stale_setup_identity_ends_preparation() {
    let mut state = VendingState::default();
    state.apply(&ingress(ready()));
    let request_id = state.begin_open(SETUP_ID, 0.0).expect("open request");
    state.apply(&ingress(VendingIngressPayload::OpenResult {
        request_id,
        setup_id: "setup-old".into(),
        setup_active: true,
        outcome: VendingOpenOutcome::InvalidState,
        snapshot: None,
    }));
    assert!(state.setup().is_none());
}

#[test]
fn unmatched_and_duplicate_replies_never_settle_or_repeat_feedback() {
    let mut state = VendingState::default();
    opened_shop(&mut state);
    state.take_notices();

    state.apply(&ingress(open_result(
        999,
        false,
        VendingOpenOutcome::Success,
    )));
    state.apply(&ingress(list_result(7, VendingListOutcome::Success, 0)));
    state.apply(&ingress(purchase_result(
        8,
        VENDOR_ID,
        SHOP_ID,
        VendingPurchaseOutcome::Success,
    )));
    state.apply(&ingress(close_result(
        9,
        VendingCloseTarget::Shop(SHOP_ID.into()),
        VendingCloseOutcome::ShopClosed,
    )));

    assert!(state.active_shop().is_some());
    assert!(state.browse().is_none());
    assert!(!state.purchase_pending());
    assert!(state.take_notices().is_empty());
}

#[test]
fn browse_supersedes_older_reads_and_ignores_answers_without_a_request() {
    let mut state = VendingState::default();
    let first = state.begin_browse(VENDOR_ID).expect("first browse");
    let second = state.begin_browse(VENDOR_ID + 1).expect("second browse");
    assert_ne!(first, second);

    state.apply(&ingress(list_result(first, VendingListOutcome::Success, 0)));
    assert!(state.browse().is_none());
    assert!(state.take_notices().is_empty());

    state.apply(&ingress(VendingIngressPayload::ListResult {
        request_id: second,
        vendor_unit_id: VENDOR_ID + 1,
        outcome: VendingListOutcome::Success,
        snapshot: Some(VendingSnapshot {
            vendor_unit_id: VENDOR_ID + 1,
            shop_id: SHOP_ID.into(),
            title: "Scrolls".into(),
            items: vec![],
            total_earned: 0,
        }),
    }));
    assert_eq!(
        state.browse().map(|s| s.vendor_unit_id),
        Some(VENDOR_ID + 1)
    );
    assert_eq!(state.take_notices().len(), 1);
}

#[test]
fn owner_snapshot_reconciles_shop_authority_after_a_map_change() {
    let mut state = VendingState::default();
    opened_shop(&mut state);
    state.take_notices();

    state.apply(&ingress(VendingIngressPayload::MapChanging));
    assert!(state.owner_authority_unconfirmed());
    assert!(state.gameplay_locked());
    assert!(state.active_shop().is_some());

    state.apply(&ingress(list_result(0, VendingListOutcome::Success, 4_321)));
    assert!(!state.owner_authority_unconfirmed());
    assert_eq!(
        state.active_shop().map(|shop| shop.total_earned),
        Some(4_321)
    );
    assert!(state.take_notices().is_empty());

    state.apply(&ingress(VendingIngressPayload::MapChanging));
    state.apply(&ingress(list_result(0, VendingListOutcome::NotOpen, 0)));
    assert!(state.active_shop().is_none());
    assert!(!state.owner_authority_unconfirmed());
}

#[test]
fn board_removal_respects_shop_identity() {
    let mut state = VendingState::default();
    state.apply(&ingress(board()));
    state.apply(&ingress(VendingIngressPayload::BoardRemoved {
        unit_id: VENDOR_ID,
        shop_id: "shop-old".into(),
        reason: VendingClosureReason::UserClosed,
    }));
    assert!(state.boards().contains_key(&VENDOR_ID));

    state.apply(&ingress(VendingIngressPayload::BoardRemoved {
        unit_id: VENDOR_ID,
        shop_id: SHOP_ID.into(),
        reason: VendingClosureReason::UserClosed,
    }));
    assert!(state.boards().is_empty());
}

#[test]
fn sold_out_closure_keeps_the_shop_for_final_reports() {
    let mut state = VendingState::default();
    opened_shop(&mut state);
    state.apply(&ingress(VendingIngressPayload::BoardRemoved {
        unit_id: VENDOR_ID,
        shop_id: SHOP_ID.into(),
        reason: VendingClosureReason::SoldOut,
    }));
    assert!(state.active_shop().is_none());
    assert!(!state.cart_transfers_locked());
    assert_eq!(
        state.last_closed_shop().map(|shop| shop.shop_id.as_str()),
        Some(SHOP_ID)
    );

    state.apply(&ingress(sale(1, 900)));
    assert_eq!(
        state.last_closed_shop().map(|shop| shop.total_earned),
        Some(900)
    );
    assert_eq!(state.recent_sales().len(), 1);

    state.apply(&ingress(sale(1, 900)));
    assert_eq!(state.recent_sales().len(), 1);
}

#[test]
fn cumulative_earnings_come_from_the_server_not_from_summation() {
    let mut state = VendingState::default();
    opened_shop(&mut state);
    state.apply(&ingress(sale(1, 500)));
    state.apply(&ingress(sale(2, 950)));
    assert_eq!(state.active_shop().map(|shop| shop.total_earned), Some(950));
    assert_eq!(state.recent_sales().len(), 2);
}

#[test]
fn sale_reports_for_other_shops_are_ignored() {
    let mut state = VendingState::default();
    state.apply(&ingress(sale(1, 500)));
    assert!(state.recent_sales().is_empty());

    opened_shop(&mut state);
    state.apply(&ingress(VendingIngressPayload::SaleReport {
        shop_id: "shop-other".into(),
        index: 2,
        nameid: 501,
        amount: 1,
        zeny_gained: 500,
        buyer_name: "Thor".into(),
        sale_sequence: 1,
        total_earned: 500,
    }));
    assert!(state.recent_sales().is_empty());
    assert_eq!(state.active_shop().map(|shop| shop.total_earned), Some(0));
}

#[test]
fn explicit_close_moves_the_shop_to_last_closed() {
    let mut state = VendingState::default();
    opened_shop(&mut state);
    state.take_notices();
    let request_id = state
        .begin_close(&VendingCloseTarget::Shop(SHOP_ID.into()), 0.0)
        .expect("close request");
    state.apply(&ingress(close_result(
        request_id,
        VendingCloseTarget::Shop(SHOP_ID.into()),
        VendingCloseOutcome::ShopClosed,
    )));

    assert!(state.active_shop().is_none());
    assert!(state.last_closed_shop().is_some());
    assert!(!state.cart_transfers_locked());
    assert!(!state.gameplay_locked());
    assert_eq!(
        state.take_notices(),
        vec![VendingNotice::Close {
            request_id,
            target: VendingCloseTarget::Shop(SHOP_ID.into()),
            outcome: VendingCloseOutcome::ShopClosed,
        }]
    );
}

#[test]
fn canceled_setup_releases_preparation() {
    let mut state = VendingState::default();
    state.apply(&ingress(ready()));
    let request_id = state
        .begin_close(&VendingCloseTarget::Setup(SETUP_ID.into()), 0.0)
        .expect("cancel request");
    state.apply(&ingress(close_result(
        request_id,
        VendingCloseTarget::Setup(SETUP_ID.into()),
        VendingCloseOutcome::SetupCanceled,
    )));
    assert!(state.setup().is_none());
    assert!(!state.cart_transfers_locked());
}

#[test]
fn rejected_close_keeps_the_current_instance() {
    let mut state = VendingState::default();
    opened_shop(&mut state);
    let request_id = state
        .begin_close(&VendingCloseTarget::Shop("shop-old".into()), 0.0)
        .expect("close request");
    state.apply(&ingress(close_result(
        request_id,
        VendingCloseTarget::Shop("shop-old".into()),
        VendingCloseOutcome::InvalidTarget,
    )));
    assert!(state.active_shop().is_some());
}

#[test]
fn one_mutation_at_a_time_per_request_kind() {
    let mut state = VendingState::default();
    state.apply(&ingress(ready()));

    assert!(state.begin_open(SETUP_ID, 0.0).is_some());
    assert!(state.begin_open(SETUP_ID, 0.0).is_none());
    let target = VendingCloseTarget::Setup(SETUP_ID.into());
    assert!(state.begin_close(&target, 0.0).is_some());
    assert!(state.begin_close(&target, 0.0).is_none());
}

#[test]
fn open_requires_the_live_setup_identity() {
    let mut state = VendingState::default();
    assert!(state.begin_open(SETUP_ID, 0.0).is_none());

    state.apply(&ingress(ready()));
    assert!(state.begin_open("setup-other", 0.0).is_none());
    assert!(state.begin_open(SETUP_ID, 0.0).is_some());
}

#[test]
fn purchase_settles_only_with_matching_request_vendor_and_shop() {
    let mut state = VendingState::default();
    opened_shop(&mut state);
    state.take_notices();
    let request_id = state
        .begin_purchase(VENDOR_ID, SHOP_ID, 0.0)
        .expect("purchase request");
    assert!(state.begin_purchase(VENDOR_ID, SHOP_ID, 0.0).is_none());

    for (id, vendor, shop) in [
        (request_id + 1, VENDOR_ID, SHOP_ID),
        (request_id, VENDOR_ID + 1, SHOP_ID),
        (request_id, VENDOR_ID, "shop-2"),
    ] {
        state.apply(&ingress(purchase_result(
            id,
            vendor,
            shop,
            VendingPurchaseOutcome::Success,
        )));
        assert!(state.purchase_pending(), "mismatched result settled");
    }

    state.apply(&ingress(purchase_result(
        request_id,
        VENDOR_ID,
        SHOP_ID,
        VendingPurchaseOutcome::NotEnoughZeny,
    )));
    assert!(!state.purchase_pending());
    assert!(state.take_refresh().is_none());
    assert_eq!(
        state.take_notices(),
        vec![VendingNotice::Purchase {
            request_id,
            vendor_unit_id: VENDOR_ID,
            shop_id: SHOP_ID.into(),
            outcome: VendingPurchaseOutcome::NotEnoughZeny,
        }]
    );
}

#[test]
fn vendor_loss_hides_browse_but_keeps_pending_money_correlation() {
    let mut state = VendingState::default();
    state.apply(&ingress(board()));
    state.begin_browse(VENDOR_ID).expect("browse");
    state
        .begin_purchase(VENDOR_ID, SHOP_ID, 0.0)
        .expect("purchase request");

    state.apply(&ingress(VendingIngressPayload::VendorDespawned {
        unit_id: VENDOR_ID,
    }));
    assert!(state.boards().is_empty());
    assert!(state.browse().is_none());
    assert!(state.purchase_pending());

    state.apply(&ingress(VendingIngressPayload::MapChanging));
    assert!(state.purchase_pending());
    assert!(state.take_notices().is_empty());
}

#[test]
fn map_change_blocks_old_read_replies_from_reopening_browsing() {
    let mut state = VendingState::default();
    state.begin_browse(VENDOR_ID).expect("browse");
    state.apply(&ingress(VendingIngressPayload::MapChanging));
    state.apply(&ingress(list_result(1, VendingListOutcome::Success, 0)));
    assert!(state.browse().is_none());

    state.reset_session();
    assert!(state.setup().is_none());
    assert!(state.active_shop().is_none());
    assert!(state.last_closed_shop().is_none());
    assert!(state.boards().is_empty());
    assert!(!state.purchase_pending());
    assert!(state.recent_sales().is_empty());
}

#[test]
fn successful_checkout_queues_a_same_shop_reload() {
    let mut state = VendingState::default();
    opened_shop(&mut state);
    let request_id = state
        .begin_purchase(VENDOR_ID, SHOP_ID, 0.0)
        .expect("purchase request");
    state.apply(&ingress(purchase_result(
        request_id,
        VENDOR_ID,
        SHOP_ID,
        VendingPurchaseOutcome::Success,
    )));

    let refresh = state.take_refresh().expect("queued refresh");
    assert_eq!(refresh.request_id, request_id + 1);
    assert_eq!(refresh.vendor_unit_id, VENDOR_ID);
    assert_eq!(refresh.expected_shop_id, SHOP_ID);
    assert!(state.take_refresh().is_none());
}

#[test]
fn unanswered_mutation_becomes_unconfirmed_once_without_unlocking() {
    let mut app = app();
    write(&mut app, ready());
    app.update();
    app.world_mut().write_message(VendingIntent::Open {
        setup_id: SETUP_ID.into(),
        title: "Potions".into(),
        entries: vec![VendingOffer {
            cart_index: 2,
            amount: 1,
            price: 5,
        }],
    });
    app.update();
    let request_id = app
        .world()
        .resource::<Messages<OpenVendingRequested>>()
        .iter_current_update_messages()
        .next()
        .map(|command| command.request_id)
        .expect("open command");
    assert!(notices(&app).is_empty());

    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f32(MUTATION_TIMEOUT_SECS + 0.5));
    app.update();
    assert_eq!(
        notices(&app),
        vec![VendingNotice::Unconfirmed { request_id }]
    );
    assert!(vending(&app).setup().is_some(), "unconfirmed is not unlock");

    app.update();
    assert!(notices(&app).is_empty(), "unconfirmed repeats");
}

#[test]
fn enter_seller_reveals_an_existing_setup_or_shop_instead_of_recasting() {
    let mut app = app();
    app.world_mut().write_message(VendingIntent::EnterSeller);
    app.update();
    let casts: Vec<_> = app
        .world()
        .resource::<Messages<SkillCastRequested>>()
        .iter_current_update_messages()
        .map(|cast| cast.skill_id)
        .collect();
    assert_eq!(casts, vec![VENDING_SKILL_ID]);
    assert!(
        app.world()
            .resource::<Messages<RevealVendingSeller>>()
            .is_empty()
    );

    write(&mut app, ready());
    app.world_mut().write_message(VendingIntent::EnterSeller);
    app.update();
    assert_eq!(
        app.world()
            .resource::<Messages<RevealVendingSeller>>()
            .iter_current_update_messages()
            .count(),
        1
    );
    assert_eq!(
        app.world()
            .resource::<Messages<SkillCastRequested>>()
            .iter_current_update_messages()
            .count(),
        0
    );
}

#[test]
fn open_intent_without_a_setup_grant_sends_nothing() {
    let mut app = app();
    app.world_mut().write_message(VendingIntent::Open {
        setup_id: SETUP_ID.into(),
        title: "Potions".into(),
        entries: vec![VendingOffer {
            cart_index: 2,
            amount: 1,
            price: 5,
        }],
    });
    app.update();
    assert!(
        app.world()
            .resource::<Messages<OpenVendingRequested>>()
            .is_empty()
    );
}

#[test]
fn browse_intent_sends_one_read_per_vendor() {
    let mut app = app();
    for _ in 0..2 {
        app.world_mut().write_message(VendingIntent::Browse {
            vendor_unit_id: VENDOR_ID,
            shop_id: SHOP_ID.into(),
        });
    }
    app.update();
    let requests: Vec<_> = app
        .world()
        .resource::<Messages<ListVendingRequested>>()
        .iter_current_update_messages()
        .cloned()
        .collect();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].vendor_unit_id, VENDOR_ID);
    assert_eq!(requests[0].expected_shop_id.as_deref(), Some(SHOP_ID));
    assert_ne!(requests[0].request_id, 0);
}

#[test]
fn refresh_after_checkout_sends_one_same_shop_read() {
    let mut app = app();
    write(&mut app, ready());
    app.update();
    app.world_mut().write_message(VendingIntent::Open {
        setup_id: SETUP_ID.into(),
        title: "Potions".into(),
        entries: vec![VendingOffer {
            cart_index: 2,
            amount: 1,
            price: 5,
        }],
    });
    app.update();
    let open_request = app
        .world()
        .resource::<Messages<OpenVendingRequested>>()
        .iter_current_update_messages()
        .next()
        .map(|command| command.request_id)
        .expect("open command");
    write(
        &mut app,
        open_result(open_request, false, VendingOpenOutcome::Success),
    );
    app.update();

    app.world_mut().write_message(VendingIntent::Purchase {
        vendor_unit_id: VENDOR_ID,
        shop_id: SHOP_ID.into(),
        items: vec![VendingBuyLine {
            index: 2,
            amount: 1,
            expected_unit_price: 5,
        }],
    });
    app.update();
    let purchase_request = app
        .world()
        .resource::<Messages<PurchaseVendingRequested>>()
        .iter_current_update_messages()
        .next()
        .map(|command| command.request_id)
        .expect("purchase command");

    write(
        &mut app,
        purchase_result(
            purchase_request,
            VENDOR_ID,
            SHOP_ID,
            VendingPurchaseOutcome::Success,
        ),
    );
    app.update();
    assert_eq!(
        notices(&app),
        vec![VendingNotice::Purchase {
            request_id: purchase_request,
            vendor_unit_id: VENDOR_ID,
            shop_id: SHOP_ID.into(),
            outcome: VendingPurchaseOutcome::Success,
        }]
    );
    let reads: Vec<_> = app
        .world()
        .resource::<Messages<ListVendingRequested>>()
        .iter_current_update_messages()
        .cloned()
        .collect();
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].expected_shop_id.as_deref(), Some(SHOP_ID));
}

#[test]
fn notices_are_projected_once_and_stale_reads_do_not_replace_a_settlement() {
    let mut app = app();
    write(&mut app, ready());
    app.update();
    app.world_mut().write_message(VendingIntent::Open {
        setup_id: SETUP_ID.into(),
        title: "Potions".into(),
        entries: vec![],
    });
    app.update();
    let open_request = app
        .world()
        .resource::<Messages<OpenVendingRequested>>()
        .iter_current_update_messages()
        .next()
        .map(|command| command.request_id)
        .expect("open command");
    write(
        &mut app,
        open_result(open_request, false, VendingOpenOutcome::Success),
    );
    app.update();
    assert_eq!(notices(&app).len(), 1);

    app.update();
    assert!(notices(&app).is_empty());
}

#[test]
fn session_generation_reset_clears_all_vending_state() {
    let mut app = app();
    write(&mut app, board());
    app.update();
    app.world_mut().write_message(VendingIntent::Browse {
        vendor_unit_id: VENDOR_ID,
        shop_id: SHOP_ID.into(),
    });
    app.update();
    assert!(vending(&app).browse().is_none());

    app.world_mut().resource_mut::<ZoneSessionGeneration>().0 = 7;
    app.update();
    assert!(vending(&app).boards().is_empty());
    assert!(vending(&app).setup().is_none());
    assert!(vending(&app).active_shop().is_none());
    assert!(!vending(&app).purchase_pending());
    assert!(
        app.world_mut()
            .resource_mut::<VendingState>()
            .take_notices()
            .is_empty()
    );
}
