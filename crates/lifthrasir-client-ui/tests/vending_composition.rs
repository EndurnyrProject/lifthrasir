//! CPU composition of the vending feature: neutral ingress into the domain
//! plugin, projected into the seller/buyer window and its feedback.
//!
//! This exercises only public contract/domain/UI APIs, so it proves the same
//! message path the adapter feeds without depending on any transport crate.

use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::cart::Cart;
use lifthrasir_client::domain::vending::{
    RevealVendingSeller, VendingIntent, VendingPlugin, VendingState,
};
use lifthrasir_client_ui::widgets::vending_window::buyer::BuyerBasket;
use lifthrasir_client_ui::widgets::vending_window::{
    VendingTitleField, VendingUi, VendingWindowPlugin, on_close_button, scene,
};
use net_contract::commands::{
    ListVendingRequested, OpenVendingRequested, PurchaseVendingRequested,
};
use net_contract::dto::{
    VendingBoard, VendingBuyLine, VendingClosureReason, VendingListOutcome, VendingListing,
    VendingOffer, VendingOpenOutcome, VendingPurchaseOutcome, VendingSetup, VendingSnapshot,
};
use net_contract::events::{ChatHeard, VendingIngress, VendingIngressPayload};
use net_contract::state::ZoneSessionGeneration;

const SETUP_ID: &str = "setup-1";
const SHOP_ID: &str = "shop-1";
const VENDOR: u64 = 5;

fn setup() -> VendingSetup {
    VendingSetup {
        setup_id: SETUP_ID.into(),
        max_slots: 12,
        min_unit_price: 0,
        max_unit_price: 1_000_000_000,
        max_title_bytes: 79,
        sellable_cart_indices: vec![2],
        sp_spent: 30,
    }
}

fn listing() -> VendingListing {
    VendingListing {
        index: 2,
        nameid: 501,
        type_: 0,
        amount: 1,
        identified: true,
        attribute: 0,
        refine: 0,
        cards: vec![],
        options: vec![],
        price: 500,
        signer_name: String::new(),
        creator_id: 0,
        creator_kind: net_contract::dto::VendingCreatorKind::None,
        enchant_grade: 0,
    }
}

fn shop_snapshot() -> VendingSnapshot {
    VendingSnapshot {
        vendor_unit_id: VENDOR,
        shop_id: SHOP_ID.into(),
        title: "Potions".into(),
        items: vec![listing()],
        total_earned: 0,
    }
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ScenePlugin,
        bevy::state::app::StatesPlugin,
        VendingPlugin,
        VendingWindowPlugin,
    ));
    app.init_state::<GameState>();
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app.init_resource::<Cart>()
        .init_resource::<ItemDb>()
        .init_resource::<InputFocus>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ZoneSessionGeneration>()
        .add_message::<lifthrasir_client::domain::skill::SkillCastRequested>()
        .add_message::<net_contract::events::ZoneDisconnected>()
        .init_resource::<Messages<ChatHeard>>()
        .add_message::<VendingIngress>()
        .add_message::<ChatHeard>()
        .add_message::<RevealVendingSeller>()
        .add_message::<OpenVendingRequested>()
        .add_message::<ListVendingRequested>()
        .add_message::<PurchaseVendingRequested>()
        .add_message::<net_contract::commands::CloseVendingRequested>();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::InGame);
    app.update();
    app.world_mut()
        .spawn_scene(scene::window())
        .expect("vending window spawns");
    app.update();
    app.update();
    app
}

fn push(app: &mut App, payload: VendingIngressPayload) {
    let generation = *app.world().resource::<ZoneSessionGeneration>();
    app.world_mut()
        .resource_mut::<Messages<VendingIngress>>()
        .write(VendingIngress {
            generation,
            payload,
        });
    app.update();
}

fn chat(app: &mut App) -> Vec<String> {
    app.world_mut()
        .resource_mut::<Messages<ChatHeard>>()
        .drain()
        .map(|line| line.message.clone())
        .collect()
}

fn state(app: &App) -> &VendingState {
    app.world().resource::<VendingState>()
}

#[test]
fn a_full_sale_cycle_settles_once_and_survives_map_cleanup() {
    let mut app = app();

    // 1. Paid setup becomes an authoritative editor.
    push(&mut app, VendingIngressPayload::SetupReady(setup()));
    assert!(state(&app).setup().is_some());
    assert!(app.world().resource::<VendingUi>().is_visible());

    // 2. The submit intent is the only mutation and settles into the monitor.
    app.world_mut().write_message(VendingIntent::Open {
        setup_id: SETUP_ID.into(),
        title: "Potions".into(),
        entries: vec![VendingOffer {
            cart_index: 2,
            amount: 1,
            price: 500,
        }],
    });
    app.update();
    let open_request = app
        .world_mut()
        .resource_mut::<Messages<OpenVendingRequested>>()
        .drain()
        .map(|command| command.request_id)
        .collect::<Vec<_>>();
    assert_eq!(open_request.len(), 1);
    push(
        &mut app,
        VendingIngressPayload::OpenResult {
            request_id: open_request[0],
            setup_id: SETUP_ID.into(),
            setup_active: false,
            outcome: VendingOpenOutcome::Success,
            snapshot: Some(shop_snapshot()),
        },
    );
    assert_eq!(
        state(&app).active_shop().map(|shop| shop.shop_id.as_str()),
        Some(SHOP_ID)
    );

    // 3. Browsing the same shop stages one confirmed checkout.
    app.world_mut().write_message(VendingIntent::Browse {
        vendor_unit_id: VENDOR,
        shop_id: SHOP_ID.into(),
    });
    app.update();
    let list_request = app
        .world_mut()
        .resource_mut::<Messages<ListVendingRequested>>()
        .drain()
        .map(|command| command.request_id)
        .collect::<Vec<_>>();
    assert_eq!(list_request.len(), 1);
    push(
        &mut app,
        VendingIngressPayload::ListResult {
            request_id: list_request[0],
            vendor_unit_id: VENDOR,
            outcome: VendingListOutcome::Success,
            snapshot: Some(shop_snapshot()),
        },
    );
    let browse = state(&app).browse().cloned().expect("browse snapshot");
    app.world_mut()
        .resource_mut::<BuyerBasket>()
        .set_quantity(&browse, 2, 1);

    app.world_mut().write_message(VendingIntent::Purchase {
        vendor_unit_id: VENDOR,
        shop_id: SHOP_ID.into(),
        items: vec![VendingBuyLine {
            index: 2,
            amount: 1,
            expected_unit_price: 500,
        }],
    });
    app.update();
    let purchase_request = app
        .world_mut()
        .resource_mut::<Messages<PurchaseVendingRequested>>()
        .drain()
        .map(|command| command.request_id)
        .collect::<Vec<_>>();
    assert_eq!(purchase_request.len(), 1, "one checkout, never two");

    // 4. Replaying the same result cannot duplicate feedback or the basket.
    assert_eq!(
        chat(&mut app),
        vec![
            "Your shop is open.".to_string(),
            "Shop stock loaded.".to_string()
        ]
    );
    for _ in 0..2 {
        push(
            &mut app,
            VendingIngressPayload::PurchaseResult {
                request_id: purchase_request[0],
                vendor_unit_id: VENDOR,
                shop_id: SHOP_ID.into(),
                outcome: VendingPurchaseOutcome::Success,
                total_charged: 500,
            },
        );
    }
    let lines = chat(&mut app);
    assert_eq!(lines, vec!["Purchase complete.".to_string()]);
    assert!(app.world().resource::<BuyerBasket>().is_empty());

    // 5. A sold-out closure keeps the shop for its final report.
    push(
        &mut app,
        VendingIngressPayload::BoardRemoved {
            unit_id: VENDOR,
            shop_id: SHOP_ID.into(),
            reason: VendingClosureReason::SoldOut,
        },
    );
    assert_eq!(
        state(&app)
            .last_closed_shop()
            .map(|shop| shop.shop_id.as_str()),
        Some(SHOP_ID)
    );
    push(
        &mut app,
        VendingIngressPayload::SaleReport {
            shop_id: SHOP_ID.into(),
            index: 2,
            nameid: 501,
            amount: 1,
            zeny_gained: 500,
            buyer_name: "Thor".into(),
            sale_sequence: 1,
            total_earned: 500,
        },
    );
    assert_eq!(
        state(&app).last_closed_shop().map(|shop| shop.total_earned),
        Some(500)
    );

    // 6. A map change keeps money correlation but distrusts owner authority,
    //    and a new session generation clears everything.
    push(&mut app, VendingIngressPayload::MapChanging);
    assert!(state(&app).owner_authority_unconfirmed());
    app.world_mut().resource_mut::<ZoneSessionGeneration>().0 += 1;
    app.update();
    assert!(state(&app).active_shop().is_none());
    assert!(state(&app).last_closed_shop().is_none());
    assert!(state(&app).boards().is_empty());
}

#[test]
fn closing_hides_an_active_shop_and_cancels_only_preparation() {
    let mut app = app();
    push(&mut app, VendingIngressPayload::SetupReady(setup()));

    let button = app.world_mut().spawn_empty().observe(on_close_button).id();
    app.world_mut().trigger(Activate { entity: button });
    let mut intents = app.world_mut().resource_mut::<Messages<VendingIntent>>();
    let written: Vec<VendingIntent> = intents.drain().collect();
    assert!(
        matches!(
            written.as_slice(),
            [VendingIntent::Close {
                target: net_contract::dto::VendingCloseTarget::Setup(id)
            }] if id == SETUP_ID
        ),
        "closing during preparation cancels the setup, got {written:?}"
    );
}

#[test]
fn a_focused_field_survives_unrelated_sales_and_board_events() {
    let mut app = app();
    push(&mut app, VendingIngressPayload::SetupReady(setup()));
    let field = app
        .world_mut()
        .spawn((VendingTitleField, EditableText::new("Draft title")))
        .id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(field, bevy::input_focus::FocusCause::Pressed);

    push(
        &mut app,
        VendingIngressPayload::BoardShown(VendingBoard {
            vendor_unit_id: VENDOR,
            shop_id: SHOP_ID.into(),
            title: "Potions".into(),
        }),
    );
    push(
        &mut app,
        VendingIngressPayload::SaleReport {
            shop_id: SHOP_ID.into(),
            index: 2,
            nameid: 501,
            amount: 1,
            zeny_gained: 500,
            buyer_name: "Thor".into(),
            sale_sequence: 1,
            total_earned: 500,
        },
    );

    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(field),
        "focus is not stolen by unrelated vending traffic"
    );
    assert_eq!(
        app.world()
            .get::<EditableText>(field)
            .map(|text| text.value().to_string()),
        Some("Draft title".to_string())
    );
}
