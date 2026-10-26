//! Vending window regressions: draft rules, affordances, structure, and feedback.

use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_client::domain::cart::Cart;
use lifthrasir_client::domain::vending::{
    RevealVendingSeller, VendingIntent, VendingNotice, VendingState,
};
use lifthrasir_data::{ItemData, ItemInfo};
use net_contract::dto::{
    CartItem, VendingCloseOutcome, VendingCloseTarget, VendingListing, VendingOpenOutcome,
    VendingSetup, VendingSnapshot,
};
use net_contract::events::{ChatHeard, VendingIngress, VendingIngressPayload};
use net_contract::state::ZoneSessionGeneration;

use super::feedback::vending_notice_text;
use super::scene;
use super::{
    DraftFieldKind, DraftRow, SellerDraft, VendingDraftField, VendingModeNote, VendingRowRemove,
    VendingStockCell, VendingTitleField, VendingUi, VendingWindowRoot, cancel_on_escape,
    deliver_vending_feedback, on_cancel_setup, on_close_shop, on_open_shop, on_row_removed,
    on_stock_toggled, rebuild_content, sync_texts, sync_window_state,
};

const SETUP_ID: &str = "setup-1";
const SHOP_ID: &str = "shop-1";

fn cart_item(index: u32, nameid: u32, amount: u32) -> CartItem {
    CartItem {
        nameid,
        index,
        amount,
        identified: true,
        refine: 0,
        cards: vec![],
        options: vec![],
        attribute: 0,
        expire_time: 0,
        weight: 10,
    }
}

fn cart() -> Cart {
    let mut cart = Cart::default();
    cart.upsert(cart_item(2, 501, 3));
    cart.upsert(cart_item(3, 2104, 1));
    cart
}

fn item_db() -> ItemDb {
    let mut data = ItemData::default();
    data.items.insert(
        501,
        ItemInfo {
            identified_name: "Red Potion".to_string(),
            identified_resource: "RED_POTION".to_string(),
            identified_description: vec![],
            unidentified_name: "Unknown Potion".to_string(),
            unidentified_resource: "UNKNOWN_POTION".to_string(),
            unidentified_description: vec![],
            slot_count: 0,
        },
    );
    data.items.insert(
        2104,
        ItemInfo {
            identified_name: "Buckler".to_string(),
            identified_resource: "BUCKLER".to_string(),
            identified_description: vec![],
            unidentified_name: "Round Shield".to_string(),
            unidentified_resource: "ROUND_SHIELD".to_string(),
            unidentified_description: vec![],
            slot_count: 1,
        },
    );
    ItemDb::from_item_data(data)
}

fn setup() -> VendingSetup {
    VendingSetup {
        setup_id: SETUP_ID.into(),
        max_slots: 12,
        min_unit_price: 0,
        max_unit_price: 1_000_000_000,
        max_title_bytes: 79,
        sellable_cart_indices: vec![2, 3],
        sp_spent: 30,
    }
}

fn snapshot() -> VendingSnapshot {
    VendingSnapshot {
        vendor_unit_id: 5,
        shop_id: SHOP_ID.into(),
        title: "Potions".into(),
        items: vec![VendingListing {
            index: 2,
            nameid: 501,
            type_: 0,
            amount: 2,
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
        }],
        total_earned: 1_500,
    }
}

fn state_with(payload: VendingIngressPayload) -> VendingState {
    let mut state = VendingState::default();
    state.apply(&VendingIngress {
        generation: ZoneSessionGeneration::default(),
        payload,
    });
    state
}

fn draft(rows: &[(u32, u32, u32)]) -> SellerDraft {
    SellerDraft {
        title: "Potions".to_string(),
        rows: rows
            .iter()
            .map(|&(cart_index, amount, price)| DraftRow {
                cart_index,
                amount,
                price,
            })
            .collect(),
    }
}

fn app(state: VendingState) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app.insert_resource(state)
        .insert_resource(cart())
        .insert_resource(item_db())
        .insert_resource(SellerDraft::default())
        .insert_resource(VendingUi::default())
        .init_resource::<InputFocus>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_message::<VendingIntent>()
        .add_message::<VendingNotice>()
        .add_message::<RevealVendingSeller>()
        .add_message::<ChatHeard>();
    app
}

fn intents(app: &mut App) -> Vec<VendingIntent> {
    app.world_mut()
        .resource_mut::<Messages<VendingIntent>>()
        .drain()
        .collect()
}

fn chat(app: &mut App) -> Vec<String> {
    app.world_mut()
        .resource_mut::<Messages<ChatHeard>>()
        .drain()
        .map(|line| line.message.clone())
        .collect()
}

#[test]
fn offers_follow_draft_order() {
    let draft = draft(&[(3, 1, 900), (2, 2, 250)]);
    let offers = draft.offers();

    assert_eq!(offers.len(), 2);
    assert_eq!(
        (offers[0].cart_index, offers[0].amount, offers[0].price),
        (3, 1, 900)
    );
    assert_eq!(
        (offers[1].cart_index, offers[1].amount, offers[1].price),
        (2, 2, 250)
    );
}

#[test]
fn toggle_adds_and_removes_one_sellable_slot_within_the_slot_cap() {
    let cart = cart();
    let mut draft = SellerDraft::default();

    let live = setup();
    draft.toggle(2, &live);
    assert_eq!(
        draft.rows,
        vec![DraftRow {
            cart_index: 2,
            amount: 1,
            price: 0
        }]
    );
    draft.toggle(3, &live);
    assert_eq!(draft.rows.len(), 2);
    draft.toggle(2, &setup());
    assert_eq!(
        draft.rows,
        vec![DraftRow {
            cart_index: 3,
            amount: 1,
            price: 0
        }]
    );

    let full = VendingSetup {
        max_slots: 1,
        ..live.clone()
    };
    draft.toggle(2, &full);
    assert_eq!(draft.rows.len(), 1, "cap already reached");
    assert!(draft.validate(&full, &cart).is_err());
}

#[test]
fn validation_rejects_counts_slots_quantities_and_prices() {
    let cart = cart();
    let live = setup();

    assert!(draft(&[]).validate(&live, &cart).is_err(), "no listings");

    let mut too_many = draft(&[]);
    too_many.rows = (0..13)
        .map(|index| DraftRow {
            cart_index: index,
            amount: 1,
            price: 0,
        })
        .collect();
    assert!(too_many.validate(&live, &cart).is_err(), "over max slots");

    assert!(
        draft(&[(2, 1, 0), (2, 1, 0)])
            .validate(&live, &cart)
            .is_err(),
        "duplicate slot"
    );
    assert!(
        draft(&[(9, 1, 0)]).validate(&live, &cart).is_err(),
        "slot is not sellable"
    );
    assert!(
        draft(&[(2, 0, 0)]).validate(&live, &cart).is_err(),
        "zero quantity"
    );
    assert!(
        draft(&[(2, 4, 0)]).validate(&live, &cart).is_err(),
        "quantity over stock"
    );

    let capped = VendingSetup {
        min_unit_price: 100,
        max_unit_price: 5_000,
        ..live.clone()
    };
    assert!(
        draft(&[(2, 1, 99)]).validate(&capped, &cart).is_err(),
        "price below the accepted minimum"
    );
    assert!(
        draft(&[(2, 1, 5_001)]).validate(&capped, &cart).is_err(),
        "price above the accepted maximum"
    );
    assert!(
        draft(&[(2, 1, 100)]).validate(&capped, &cart).is_ok(),
        "inclusive minimum"
    );
    assert!(
        draft(&[(2, 1, 5_000)]).validate(&capped, &cart).is_ok(),
        "inclusive maximum"
    );
    assert!(
        draft(&[(2, 1, 0)]).validate(&live, &cart).is_ok(),
        "zero price is allowed by policy"
    );
}

#[test]
fn validation_rejects_empty_control_and_oversized_titles() {
    let cart = cart();
    let mut draft = draft(&[(2, 1, 0)]);

    draft.title = "   ".into();
    assert!(draft.validate(&setup(), &cart).is_err(), "blank title");

    draft.title = "Pot\u{7}ions".into();
    assert!(draft.validate(&setup(), &cart).is_err(), "control byte");

    let tight = VendingSetup {
        max_title_bytes: 4,
        ..setup()
    };
    draft.title = "Potions".into();
    assert!(draft.validate(&tight, &cart).is_err(), "over byte limit");
}

#[test]
fn titles_are_clamped_on_a_character_boundary() {
    assert_eq!(super::clamp_title("Potions", 79), "Potions");
    assert_eq!(super::clamp_title("Potions", 3), "Pot");
    assert_eq!(super::clamp_title("Potion\u{e9}s", 7), "Potion");
    assert_eq!(
        super::clamp_title("\u{4fbf}\u{5229}\u{5e97}", 3),
        "\u{4fbf}"
    );
    assert_eq!(
        super::clamp_title("\u{4fbf}\u{5229}\u{5e97}", 4),
        "\u{4fbf}"
    );
    assert!(super::clamp_title("\u{4fbf}\u{5229}\u{5e97}", 4).is_char_boundary(3));
}

fn spawn_window(app: &mut App) {
    app.add_systems(
        Update,
        (sync_window_state, rebuild_content, sync_texts).chain(),
    );
    app.world_mut()
        .spawn_scene(scene::window())
        .expect("vending window scene spawns");
    app.update();
}

fn count<M: Component>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), With<M>>()
        .iter(app.world())
        .count()
}

fn all_hidden(world: &mut World) -> bool {
    world
        .query_filtered::<&Visibility, With<VendingWindowRoot>>()
        .iter(world)
        .all(|visibility| *visibility == Visibility::Hidden)
}

fn texts<M: Component>(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut query = world.query_filtered::<&Text, With<M>>();
    query.iter(world).map(|text| text.0.clone()).collect()
}

#[test]
fn stock_selection_toggles_drafted_rows() {
    let mut app = app(state_with(VendingIngressPayload::SetupReady(setup())));
    let cell = app
        .world_mut()
        .spawn(VendingStockCell { cart_index: 2 })
        .observe(on_stock_toggled)
        .id();

    app.world_mut().trigger(Activate { entity: cell });
    assert_eq!(
        app.world().resource::<SellerDraft>().rows,
        vec![DraftRow {
            cart_index: 2,
            amount: 1,
            price: 0
        }]
    );

    app.world_mut().trigger(Activate { entity: cell });
    assert!(app.world().resource::<SellerDraft>().rows.is_empty());
}

#[test]
fn a_drafted_row_can_be_removed() {
    let mut app = app(state_with(VendingIngressPayload::SetupReady(setup())));
    app.world_mut().resource_mut::<SellerDraft>().rows = vec![
        DraftRow {
            cart_index: 2,
            amount: 2,
            price: 100,
        },
        DraftRow {
            cart_index: 3,
            amount: 1,
            price: 0,
        },
    ];
    let remove = app
        .world_mut()
        .spawn(VendingRowRemove { cart_index: 2 })
        .observe(on_row_removed)
        .id();

    app.world_mut().trigger(Activate { entity: remove });
    assert_eq!(
        app.world().resource::<SellerDraft>().rows,
        vec![DraftRow {
            cart_index: 3,
            amount: 1,
            price: 0
        }]
    );
}

fn spawn_fields(app: &mut App, title: &str, rows: &[(u32, &str, &str)]) {
    app.world_mut()
        .spawn((VendingTitleField, EditableText::new(title)));
    for (cart_index, amount, price) in rows {
        app.world_mut().spawn((
            VendingDraftField {
                cart_index: *cart_index,
                kind: DraftFieldKind::Amount,
            },
            EditableText::new(*amount),
        ));
        app.world_mut().spawn((
            VendingDraftField {
                cart_index: *cart_index,
                kind: DraftFieldKind::Price,
            },
            EditableText::new(*price),
        ));
    }
}

#[test]
fn typed_title_quantity_and_price_reach_the_open_intent() {
    let mut app = app(state_with(VendingIngressPayload::SetupReady(setup())));
    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, 0), (3, 1, 0)]).rows;
    spawn_fields(&mut app, "Potions", &[(2, "3", "450"), (3, "1", "0")]);
    let submit = app.world_mut().spawn_empty().observe(on_open_shop).id();

    app.world_mut().trigger(Activate { entity: submit });

    let intents = intents(&mut app);
    assert_eq!(intents.len(), 1);
    let VendingIntent::Open {
        setup_id,
        title,
        entries,
    } = &intents[0]
    else {
        panic!("expected an open intent, got {:?}", intents[0]);
    };
    assert_eq!(setup_id, SETUP_ID);
    assert_eq!(title, "Potions");
    assert_eq!(entries.len(), 2);
    assert_eq!(
        (entries[0].cart_index, entries[0].amount, entries[0].price),
        (2, 3, 450)
    );
    assert_eq!(
        (entries[1].cart_index, entries[1].amount, entries[1].price),
        (3, 1, 0)
    );
    assert!(app.world().resource::<VendingUi>().error().is_none());
}

#[test]
fn an_invalid_draft_reports_instead_of_submitting() {
    let mut app = app(state_with(VendingIngressPayload::SetupReady(setup())));
    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, 0)]).rows;
    spawn_fields(&mut app, "Potions", &[(2, "4", "0")]);
    let submit = app.world_mut().spawn_empty().observe(on_open_shop).id();

    app.world_mut().trigger(Activate { entity: submit });

    assert!(intents(&mut app).is_empty());
    assert_eq!(
        app.world().resource::<VendingUi>().error(),
        Some("Quantity must be between 1 and 3.")
    );
}

#[test]
fn submission_waits_for_an_unanswered_open() {
    let mut state = state_with(VendingIngressPayload::SetupReady(setup()));
    assert!(state.begin_open(SETUP_ID, 0.0).is_some());
    let mut app = app(state);
    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, 0)]).rows;
    spawn_fields(&mut app, "Potions", &[(2, "1", "0")]);
    let submit = app.world_mut().spawn_empty().observe(on_open_shop).id();

    app.world_mut().trigger(Activate { entity: submit });
    assert!(intents(&mut app).is_empty());
}

#[test]
fn cancel_and_escape_cancel_preparation_but_never_close_an_active_shop() {
    let mut app = app(state_with(VendingIngressPayload::SetupReady(setup())));
    let cancel = app.world_mut().spawn_empty().observe(on_cancel_setup).id();
    app.world_mut().trigger(Activate { entity: cancel });
    assert_eq!(
        intents(&mut app),
        vec![VendingIntent::Close {
            target: VendingCloseTarget::Setup(SETUP_ID.into()),
        }]
    );

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.add_systems(Update, cancel_on_escape);
    app.update();
    assert!(
        intents(&mut app).is_empty(),
        "one cancel is already pending"
    );
}

#[test]
fn escape_and_close_hide_an_active_shop_without_closing_it() {
    let mut app = app(state_with(VendingIngressPayload::OpenResult {
        request_id: 1,
        setup_id: SETUP_ID.into(),
        setup_active: false,
        outcome: VendingOpenOutcome::Success,
        snapshot: Some(snapshot()),
    }));
    let mut state = VendingState::default();
    state.apply(&VendingIngress {
        generation: ZoneSessionGeneration::default(),
        payload: VendingIngressPayload::SetupReady(setup()),
    });
    let request_id = state.begin_open(SETUP_ID, 0.0).expect("open request");
    state.apply(&VendingIngress {
        generation: ZoneSessionGeneration::default(),
        payload: VendingIngressPayload::OpenResult {
            request_id,
            setup_id: SETUP_ID.into(),
            setup_active: false,
            outcome: VendingOpenOutcome::Success,
            snapshot: Some(snapshot()),
        },
    });
    *app.world_mut().resource_mut::<VendingState>() = state;
    app.world_mut().resource_mut::<VendingUi>().visible = true;

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.add_systems(Update, cancel_on_escape);
    app.update();

    assert!(
        intents(&mut app).is_empty(),
        "escape must not close the shop"
    );
    assert!(!app.world().resource::<VendingUi>().is_visible());
}

#[test]
fn explicit_close_waits_for_the_result() {
    let mut app = app(state_with(VendingIngressPayload::OpenResult {
        request_id: 1,
        setup_id: SETUP_ID.into(),
        setup_active: false,
        outcome: VendingOpenOutcome::Success,
        snapshot: Some(snapshot()),
    }));
    let mut state = VendingState::default();
    state.apply(&VendingIngress {
        generation: ZoneSessionGeneration::default(),
        payload: VendingIngressPayload::SetupReady(setup()),
    });
    let request_id = state.begin_open(SETUP_ID, 0.0).expect("open request");
    state.apply(&VendingIngress {
        generation: ZoneSessionGeneration::default(),
        payload: VendingIngressPayload::OpenResult {
            request_id,
            setup_id: SETUP_ID.into(),
            setup_active: false,
            outcome: VendingOpenOutcome::Success,
            snapshot: Some(snapshot()),
        },
    });
    *app.world_mut().resource_mut::<VendingState>() = state;
    app.world_mut().resource_mut::<VendingUi>().visible = true;

    let close = app.world_mut().spawn_empty().observe(on_close_shop).id();
    app.world_mut().trigger(Activate { entity: close });
    assert_eq!(
        intents(&mut app),
        vec![VendingIntent::Close {
            target: VendingCloseTarget::Shop(SHOP_ID.into()),
        }]
    );

    app.world_mut().trigger(Activate { entity: close });
    assert!(
        intents(&mut app).is_empty(),
        "a second close waits for the result"
    );
}

#[test]
fn notices_become_exactly_one_chat_line_each() {
    let mut app = app(VendingState::default());
    app.add_systems(Update, deliver_vending_feedback);
    let notice = VendingNotice::Close {
        request_id: 3,
        target: VendingCloseTarget::Setup(SETUP_ID.into()),
        outcome: VendingCloseOutcome::SetupCanceled,
    };
    app.world_mut()
        .resource_mut::<Messages<VendingNotice>>()
        .write(notice.clone());

    app.update();
    assert_eq!(chat(&mut app), vec![vending_notice_text(&notice)]);

    app.update();
    assert!(chat(&mut app).is_empty(), "a notice is delivered once");
}

#[test]
fn the_editor_only_appears_with_an_authoritative_setup() {
    let mut idle = app(VendingState::default());
    spawn_window(&mut idle);
    assert!(!idle.world().resource::<VendingUi>().is_visible());
    assert_eq!(count::<VendingStockCell>(&mut idle), 0);
    assert!(all_hidden(idle.world_mut()));

    let mut app = app(state_with(VendingIngressPayload::SetupReady(setup())));
    spawn_window(&mut app);
    assert_eq!(
        texts::<VendingModeNote>(&mut app),
        vec!["SP already paid: 30"]
    );
    assert_eq!(texts::<super::VendingTitle>(&mut app), vec!["Set Up Shop"]);
    assert!(!all_hidden(app.world_mut()));
}

#[test]
fn one_stock_cell_per_sellable_slot_and_one_row_per_drafted_slot() {
    let mut app = app(state_with(VendingIngressPayload::SetupReady(setup())));
    spawn_window(&mut app);
    assert_eq!(
        count::<VendingStockCell>(&mut app),
        2,
        "both sellable slots"
    );
    assert_eq!(count::<VendingDraftField>(&mut app), 0);

    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, 0)]).rows;
    app.update();
    assert_eq!(count::<VendingRowRemove>(&mut app), 1);
    assert_eq!(
        count::<VendingDraftField>(&mut app),
        2,
        "quantity and price"
    );

    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, 0), (3, 1, 0)]).rows;
    app.update();
    assert_eq!(count::<VendingRowRemove>(&mut app), 2);
    assert_eq!(count::<VendingDraftField>(&mut app), 4);
}

#[test]
fn typed_fields_survive_unrelated_updates() {
    let mut app = app(state_with(VendingIngressPayload::SetupReady(setup())));
    spawn_window(&mut app);
    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, 0)]).rows;
    app.update();

    let mut field = app
        .world_mut()
        .query_filtered::<Entity, With<VendingDraftField>>();
    let before: Vec<Entity> = field.iter(app.world()).collect();
    assert_eq!(before.len(), 2);

    // An unrelated cart edit must not respawn (and unfocus) the typed fields.
    app.world_mut()
        .resource_mut::<Cart>()
        .upsert(cart_item(7, 501, 9));
    app.update();

    let mut field = app
        .world_mut()
        .query_filtered::<Entity, With<VendingDraftField>>();
    let after: Vec<Entity> = field.iter(app.world()).collect();
    assert_eq!(
        after, before,
        "typed fields are kept while the set is stable"
    );
}

#[test]
fn an_accepted_shop_replaces_the_editor_with_the_monitor() {
    let mut state = state_with(VendingIngressPayload::SetupReady(setup()));
    let request_id = state.begin_open(SETUP_ID, 0.0).expect("open request");
    state.apply(&VendingIngress {
        generation: ZoneSessionGeneration::default(),
        payload: VendingIngressPayload::OpenResult {
            request_id,
            setup_id: SETUP_ID.into(),
            setup_active: false,
            outcome: VendingOpenOutcome::Success,
            snapshot: Some(snapshot()),
        },
    });
    state.apply(&VendingIngress {
        generation: ZoneSessionGeneration::default(),
        payload: VendingIngressPayload::SaleReport {
            shop_id: SHOP_ID.into(),
            index: 2,
            nameid: 501,
            amount: 1,
            zeny_gained: 500,
            buyer_name: "Thor".into(),
            sale_sequence: 1,
            total_earned: 1_500,
        },
    });

    let mut app = app(state);
    spawn_window(&mut app);
    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, 0)]).rows;
    app.update();

    assert_eq!(
        count::<VendingStockCell>(&mut app),
        0,
        "monitor is read-only"
    );
    assert_eq!(count::<VendingDraftField>(&mut app), 0);
    assert_eq!(texts::<VendingModeNote>(&mut app), vec!["Earned 1500z"]);
    assert_eq!(texts::<super::VendingTitle>(&mut app), vec!["My Shop"]);

    let world = app.world_mut();
    let mut query = world.query::<&Text>();
    let rendered: Vec<String> = query.iter(world).map(|text| text.0.clone()).collect();
    assert!(rendered.iter().any(|line| line == "Red Potion"));
    assert!(rendered.iter().any(|line| line.contains("500z")));
    assert!(
        rendered.iter().any(|line| line.contains("Thor")),
        "recent sales are listed"
    );
}

#[test]
fn a_settled_close_makes_the_affordance_usable_again() {
    let mut state = VendingState::default();
    state.apply(&VendingIngress {
        generation: ZoneSessionGeneration::default(),
        payload: VendingIngressPayload::SetupReady(setup()),
    });
    let mut app = app(state);
    app.add_systems(Update, sync_window_state);

    let cancel = app.world_mut().spawn_empty().observe(on_cancel_setup).id();
    app.world_mut().trigger(Activate { entity: cancel });
    assert_eq!(intents(&mut app).len(), 1);

    app.world_mut().trigger(Activate { entity: cancel });
    assert!(
        intents(&mut app).is_empty(),
        "one cancel waits for the result"
    );

    // Nothing is pending in the domain here, so the next frame re-enables it.
    app.update();
    app.world_mut().trigger(Activate { entity: cancel });
    assert_eq!(intents(&mut app).len(), 1);
}
