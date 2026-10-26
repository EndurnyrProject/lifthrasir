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
    VendingPurchaseOutcome, VendingSetup, VendingSnapshot,
};
use net_contract::events::{ChatHeard, VendingIngress, VendingIngressPayload};
use net_contract::state::ZoneSessionGeneration;

use super::drag::{DragKind, DragSource, DropAction, DropZone, VendingDrag};
use super::feedback::vending_notice_text;
use super::scene;
use super::{
    DraftFieldKind, DraftRow, SellerDraft, VendingDraftField, VendingModeNote, VendingRowRemove,
    VendingTitleField, VendingUi, VendingWindowRoot, cancel_on_escape, deliver_vending_feedback,
    on_cancel_setup, on_close_shop, on_open_shop, on_row_removed, rebuild_content, sync_texts,
    sync_window_state,
};

const SETUP_ID: &str = "setup-1";
const VENDOR_ID: u64 = 5;
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
        .init_resource::<super::buyer::BuyerBasket>()
        .insert_resource(VendingUi::default())
        .init_resource::<VendingDrag>()
        .init_resource::<super::drag::LastVendingClick>()
        .init_resource::<InputFocus>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_message::<VendingIntent>()
        .add_message::<VendingNotice>()
        .add_message::<RevealVendingSeller>()
        .add_message::<ChatHeard>()
        .add_message::<super::ShowInfoModal>();
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
fn add_lists_the_whole_stack_once_within_the_slot_cap() {
    let mut draft = SellerDraft::default();
    let live = setup();

    assert_eq!(draft.add(2, 3, &live), Ok(true));
    assert_eq!(
        draft.rows,
        vec![DraftRow {
            cart_index: 2,
            amount: 3,
            price: 0
        }],
        "the whole stack at the minimum price"
    );
    assert_eq!(draft.add(2, 3, &live), Ok(false), "already listed");
    assert!(draft.add(9, 1, &live).is_err(), "not a sellable slot");

    let full = VendingSetup {
        max_slots: 1,
        ..live.clone()
    };
    assert!(draft.add(3, 1, &full).is_err(), "cap already reached");
    assert_eq!(draft.rows.len(), 1);

    assert!(draft.remove(2));
    assert!(!draft.remove(2), "removing twice is a no-op");
    assert!(draft.rows.is_empty());
}

#[test]
fn each_drop_zone_accepts_only_its_own_payload() {
    let kinds = [
        DragKind::Stock,
        DragKind::Draft,
        DragKind::Listing,
        DragKind::Basket,
    ];
    let zones = [
        DropZone::SetupListings,
        DropZone::CartStock,
        DropZone::Basket,
        DropZone::VendorListing,
    ];
    for zone in zones {
        for kind in kinds {
            let action = super::drag::drop_action(zone, DragSource { kind, index: 7 });
            let expected = match (zone, kind) {
                (DropZone::SetupListings, DragKind::Stock) => Some(DropAction::List(7)),
                (DropZone::CartStock, DragKind::Draft) => Some(DropAction::Unlist(7)),
                (DropZone::Basket, DragKind::Listing) => Some(DropAction::AddToBasket(7)),
                (DropZone::VendorListing, DragKind::Basket) => {
                    Some(DropAction::RemoveFromBasket(7))
                }
                _ => None,
            };
            assert_eq!(action, expected, "{zone:?} <- {kind:?}");
        }
    }

    let shortcut = |kind| super::drag::shortcut_action(DragSource { kind, index: 7 });
    assert_eq!(shortcut(DragKind::Stock), Some(DropAction::List(7)));
    assert_eq!(
        shortcut(DragKind::Listing),
        Some(DropAction::AddToBasket(7))
    );
    assert_eq!(shortcut(DragKind::Draft), None, "rows have no double-click");
    assert_eq!(shortcut(DragKind::Basket), None);
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
        (
            sync_window_state,
            rebuild_content,
            super::source::rebuild_source,
            super::focus_new_price_field,
            sync_texts,
        )
            .chain(),
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

fn source_entity(app: &mut App, kind: DragKind, index: u32) -> Entity {
    let world = app.world_mut();
    world
        .query::<(Entity, &DragSource)>()
        .iter(world)
        .find(|(_, source)| **source == DragSource { kind, index })
        .map(|(entity, _)| entity)
        .unwrap_or_else(|| panic!("no {kind:?} source for {index}"))
}

fn sources(app: &mut App, kind: DragKind) -> usize {
    let world = app.world_mut();
    world
        .query::<&DragSource>()
        .iter(world)
        .filter(|source| source.kind == kind)
        .count()
}

fn zone_entity(app: &mut App, zone: DropZone) -> Entity {
    let world = app.world_mut();
    world
        .query::<(Entity, &DropZone)>()
        .iter(world)
        .find(|(_, found)| **found == zone)
        .map(|(entity, _)| entity)
        .unwrap_or_else(|| panic!("no {zone:?} zone"))
}

fn draft_rows(app: &App) -> Vec<DraftRow> {
    app.world().resource::<SellerDraft>().rows.clone()
}

#[test]
fn dragging_stock_onto_the_listings_lists_it_and_back_unlists_it() {
    let mut app = app(state_with(VendingIngressPayload::SetupReady(setup())));
    spawn_window(&mut app);
    let window = app.world_mut().spawn(bevy::window::Window::default()).id();

    let stock = source_entity(&mut app, DragKind::Stock, 2);
    app.world_mut().trigger(drag_start(stock, window));
    assert_eq!(
        app.world().resource::<VendingDrag>().payload,
        Some(DragSource {
            kind: DragKind::Stock,
            index: 2
        })
    );

    // The drop lands on the hint inside the zone and bubbles up to it.
    let zone = zone_entity(&mut app, DropZone::SetupListings);
    let hint = app
        .world()
        .entity(zone)
        .get::<Children>()
        .expect("drop hint")[0];
    app.world_mut().trigger(drag_drop(hint, stock, window));
    assert_eq!(
        draft_rows(&app),
        vec![DraftRow {
            cart_index: 2,
            amount: 3,
            price: 0
        }]
    );

    app.update();
    let price = {
        let world = app.world_mut();
        world
            .query::<(Entity, &VendingDraftField)>()
            .iter(world)
            .find(|(_, field)| field.cart_index == 2 && field.kind == DraftFieldKind::Price)
            .map(|(entity, _)| entity)
            .expect("price field")
    };
    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(price),
        "the new row's price takes focus"
    );

    let handle = source_entity(&mut app, DragKind::Draft, 2);
    app.world_mut().trigger(drag_start(handle, window));
    let stock_panel = zone_entity(&mut app, DropZone::CartStock);
    app.world_mut()
        .trigger(drag_drop(stock_panel, handle, window));
    assert!(draft_rows(&app).is_empty(), "dropped back on the stock");
}

#[test]
fn a_double_clicked_stock_cell_lists_it_like_a_drop() {
    let mut app = app(state_with(VendingIngressPayload::SetupReady(setup())));
    spawn_window(&mut app);
    let window = app.world_mut().spawn(bevy::window::Window::default()).id();
    let stock = source_entity(&mut app, DragKind::Stock, 3);

    app.world_mut()
        .trigger(click_event(stock, window, PointerButton::Primary));
    assert!(draft_rows(&app).is_empty(), "a single click only selects");

    app.world_mut()
        .trigger(click_event(stock, window, PointerButton::Primary));
    assert_eq!(
        draft_rows(&app),
        vec![DraftRow {
            cart_index: 3,
            amount: 1,
            price: 0
        }]
    );
}

#[test]
fn a_full_shop_refuses_a_drop_with_a_message() {
    let full = VendingSetup {
        max_slots: 1,
        ..setup()
    };
    let mut app = app(state_with(VendingIngressPayload::SetupReady(full)));
    spawn_window(&mut app);
    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, 0)]).rows;
    let window = app.world_mut().spawn(bevy::window::Window::default()).id();
    let stock = source_entity(&mut app, DragKind::Stock, 3);

    app.world_mut()
        .trigger(click_event(stock, window, PointerButton::Primary));
    app.world_mut()
        .trigger(click_event(stock, window, PointerButton::Primary));
    assert_eq!(draft_rows(&app).len(), 1);
    assert_eq!(
        app.world().resource::<VendingUi>().error(),
        Some("A shop can list at most 1 items.")
    );
}

#[test]
fn a_released_button_ends_any_drag() {
    let mut app = app(VendingState::default());
    app.add_systems(Update, super::drag::end_drag_on_release);
    let payload = Some(DragSource::default());

    app.init_resource::<ButtonInput<MouseButton>>();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut().resource_mut::<VendingDrag>().payload = payload;
    app.update();
    assert_eq!(app.world().resource::<VendingDrag>().payload, payload);

    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    assert_eq!(app.world().resource::<VendingDrag>().payload, None);
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
    assert_eq!(sources(&mut idle, DragKind::Stock), 0);
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
    assert_eq!(sources(&mut app, DragKind::Stock), 2, "both sellable slots");
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
        sources(&mut app, DragKind::Stock),
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

fn browse_state(items: Vec<VendingListing>) -> VendingState {
    let mut state = VendingState::default();
    state.begin_browse(VENDOR_ID).expect("browse request");
    state.apply(&VendingIngress {
        generation: ZoneSessionGeneration::default(),
        payload: VendingIngressPayload::ListResult {
            request_id: 1,
            vendor_unit_id: VENDOR_ID,
            outcome: net_contract::dto::VendingListOutcome::Success,
            snapshot: Some(VendingSnapshot {
                vendor_unit_id: VENDOR_ID,
                shop_id: SHOP_ID.into(),
                title: "Potions".into(),
                items,
                total_earned: 0,
            }),
        },
    });
    state
}

fn buyable(index: u32, amount: u32, price: u32) -> VendingListing {
    VendingListing {
        index,
        nameid: 501,
        type_: 0,
        amount,
        identified: true,
        attribute: 0,
        refine: 0,
        cards: vec![],
        options: vec![],
        price,
        signer_name: String::new(),
        creator_id: 0,
        creator_kind: net_contract::dto::VendingCreatorKind::None,
        enchant_grade: 0,
    }
}

fn with_zeny(app: &mut App, zeny: u32) {
    app.world_mut().spawn((
        lifthrasir_client::domain::entities::markers::LocalPlayer,
        lifthrasir_client::domain::entities::character::components::status::CharacterStatus {
            zeny,
            ..Default::default()
        },
    ));
}

fn click_event(target: Entity, window: Entity, button: PointerButton) -> PointerClick {
    use bevy::camera::NormalizedRenderTarget;
    use bevy::picking::backend::HitData;
    use bevy::picking::pointer::{Location, PointerId};
    use bevy::window::WindowRef;

    PointerClick {
        entity: target,
        pointer: Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary.normalize(Some(window)).unwrap(),
                ),
                position: Vec2::ZERO,
            },
        ),
        button,
        hit: HitData::new(target, 0.0, None, None),
        duration: std::time::Duration::ZERO,
        count: 1,
    }
}

fn primary_pointer(window: Entity) -> Pointer {
    use bevy::camera::NormalizedRenderTarget;
    use bevy::picking::pointer::{Location, PointerId};
    use bevy::window::WindowRef;

    Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Primary.normalize(Some(window)).unwrap(),
            ),
            position: Vec2::ZERO,
        },
    )
}

fn drag_start(target: Entity, window: Entity) -> PointerDragStart {
    PointerDragStart {
        entity: target,
        pointer: primary_pointer(window),
        button: PointerButton::Primary,
        hit: bevy::picking::backend::HitData::new(target, 0.0, None, None),
    }
}

fn drag_drop(target: Entity, dropped: Entity, window: Entity) -> PointerDragDrop {
    PointerDragDrop {
        entity: target,
        pointer: primary_pointer(window),
        button: PointerButton::Primary,
        dropped,
        hit: bevy::picking::backend::HitData::new(target, 0.0, None, None),
    }
}

#[test]
fn checkout_emits_one_confirmed_basket_quoted_from_the_shop() {
    let mut app = app(browse_state(vec![buyable(2, 3, 500), buyable(3, 1, 750)]));
    with_zeny(&mut app, 2_000);
    app.add_systems(Update, super::sync_browse_basket);
    app.update();

    for (index, up) in [(2, true), (2, true), (3, true)] {
        let step = app
            .world_mut()
            .spawn(super::BasketStep { index, up })
            .observe(super::on_basket_step)
            .id();
        app.world_mut().trigger(Activate { entity: step });
    }
    let shop = app
        .world()
        .resource::<VendingState>()
        .browse()
        .unwrap()
        .clone();
    assert_eq!(
        app.world()
            .resource::<super::buyer::BuyerBasket>()
            .total(&shop),
        Some(1_750)
    );

    let checkout = app
        .world_mut()
        .spawn_empty()
        .observe(super::on_checkout)
        .id();
    app.world_mut().trigger(Activate { entity: checkout });

    let written = intents(&mut app);
    assert_eq!(written.len(), 1);
    let VendingIntent::Purchase {
        vendor_unit_id,
        shop_id,
        items,
    } = &written[0]
    else {
        panic!("expected a purchase intent, got {:?}", written[0]);
    };
    assert_eq!((*vendor_unit_id, shop_id.as_str()), (VENDOR_ID, SHOP_ID));
    assert_eq!(items.len(), 2);
    assert_eq!(
        (
            items[0].index,
            items[0].amount,
            items[0].expected_unit_price
        ),
        (2, 2, 500)
    );
    assert_eq!(
        (
            items[1].index,
            items[1].amount,
            items[1].expected_unit_price
        ),
        (3, 1, 750)
    );

    // While the domain tracks that purchase, a second confirmation is inert.
    assert!(
        app.world_mut()
            .resource_mut::<VendingState>()
            .begin_purchase(VENDOR_ID, SHOP_ID, 0.0)
            .is_some()
    );
    app.world_mut().trigger(Activate { entity: checkout });
    assert!(intents(&mut app).is_empty());
}

#[test]
fn checkout_refuses_an_empty_basket_and_short_funds() {
    let mut app = app(browse_state(vec![buyable(2, 3, 500)]));
    with_zeny(&mut app, 200);
    let checkout = app
        .world_mut()
        .spawn_empty()
        .observe(super::on_checkout)
        .id();
    app.world_mut().trigger(Activate { entity: checkout });
    assert!(intents(&mut app).is_empty());
    assert_eq!(
        app.world().resource::<VendingUi>().error(),
        Some("Choose at least one item.")
    );

    let shop = app
        .world()
        .resource::<VendingState>()
        .browse()
        .unwrap()
        .clone();
    app.world_mut()
        .resource_mut::<super::buyer::BuyerBasket>()
        .set_quantity(&shop, 2, 1);
    app.world_mut().trigger(Activate { entity: checkout });
    assert!(intents(&mut app).is_empty(), "200z cannot buy a 500z item");
    assert_eq!(
        app.world().resource::<VendingUi>().error(),
        Some("You need 300 more zeny.")
    );
}

#[test]
fn a_settled_success_clears_the_basket_and_a_failure_keeps_it() {
    let mut app = app(browse_state(vec![buyable(2, 3, 500)]));
    app.add_systems(
        Update,
        (super::sync_browse_basket, super::settle_buyer_basket).chain(),
    );
    app.update();
    let shop = app
        .world()
        .resource::<VendingState>()
        .browse()
        .unwrap()
        .clone();
    app.world_mut()
        .resource_mut::<super::buyer::BuyerBasket>()
        .set_quantity(&shop, 2, 2);
    assert_eq!(
        app.world()
            .resource::<super::buyer::BuyerBasket>()
            .lines()
            .len(),
        1
    );

    app.world_mut()
        .resource_mut::<Messages<VendingNotice>>()
        .write(VendingNotice::Purchase {
            request_id: 4,
            vendor_unit_id: VENDOR_ID,
            shop_id: SHOP_ID.into(),
            outcome: VendingPurchaseOutcome::NotEnoughZeny,
        });
    app.update();
    assert_eq!(
        app.world()
            .resource::<super::buyer::BuyerBasket>()
            .lines()
            .len(),
        1,
        "a failed checkout keeps the basket"
    );

    app.world_mut()
        .resource_mut::<Messages<VendingNotice>>()
        .write(VendingNotice::Purchase {
            request_id: 5,
            vendor_unit_id: VENDOR_ID,
            shop_id: SHOP_ID.into(),
            outcome: VendingPurchaseOutcome::Success,
        });
    app.update();
    assert!(
        app.world()
            .resource::<super::buyer::BuyerBasket>()
            .is_empty(),
        "a completed checkout clears the basket"
    );
}

#[test]
fn a_refreshed_shop_clamps_the_basket_and_explains_it() {
    let mut app = app(browse_state(vec![buyable(2, 3, 500)]));
    with_zeny(&mut app, 2_000);
    app.add_systems(Update, super::sync_browse_basket);
    app.update();

    let shop = app
        .world()
        .resource::<VendingState>()
        .browse()
        .unwrap()
        .clone();
    app.world_mut()
        .resource_mut::<super::buyer::BuyerBasket>()
        .set_quantity(&shop, 2, 3);

    let refresh = app
        .world_mut()
        .resource_mut::<VendingState>()
        .begin_browse(VENDOR_ID)
        .expect("refresh request");
    {
        let mut state = app.world_mut().resource_mut::<VendingState>();
        state.apply(&VendingIngress {
            generation: ZoneSessionGeneration::default(),
            payload: VendingIngressPayload::ListResult {
                request_id: refresh,
                vendor_unit_id: VENDOR_ID,
                outcome: net_contract::dto::VendingListOutcome::Success,
                snapshot: Some(VendingSnapshot {
                    vendor_unit_id: VENDOR_ID,
                    shop_id: SHOP_ID.into(),
                    title: "Potions".into(),
                    items: vec![buyable(2, 1, 500)],
                    total_earned: 0,
                }),
            },
        });
    }
    app.update();

    assert_eq!(
        app.world()
            .resource::<super::buyer::BuyerBasket>()
            .quantity(2),
        1,
        "reduced stock is clamped"
    );
    assert_eq!(
        app.world().resource::<VendingUi>().error(),
        Some("Only 1 of Item #501 remain.")
    );
}

fn basket_quantity(app: &App, index: u32) -> u32 {
    app.world()
        .resource::<super::buyer::BuyerBasket>()
        .quantity(index)
}

#[test]
fn a_listing_double_click_stages_and_right_click_inspects_without_bag_actions() {
    let mut app = app(browse_state(vec![buyable(2, 3, 500)]));
    spawn_window(&mut app);
    let window = app.world_mut().spawn(bevy::window::Window::default()).id();
    let listing = source_entity(&mut app, DragKind::Listing, 2);

    app.world_mut()
        .trigger(click_event(listing, window, PointerButton::Primary));
    assert_eq!(basket_quantity(&app, 2), 0, "a single click only selects");
    app.world_mut()
        .trigger(click_event(listing, window, PointerButton::Primary));
    assert_eq!(basket_quantity(&app, 2), 1);

    app.world_mut()
        .trigger(click_event(listing, window, PointerButton::Secondary));
    let written: Vec<super::ShowInfoModal> = app
        .world_mut()
        .resource_mut::<Messages<super::ShowInfoModal>>()
        .drain()
        .collect();
    assert_eq!(written.len(), 1);
    let super::InfoContent::Item { view, action } = &written[0].content else {
        panic!("expected an item view");
    };
    assert_eq!(view.name, "Red Potion");
    assert_eq!(view.meta[0], ("Price".to_string(), "500z".to_string()));
    assert!(action.is_none(), "vendor stock offers no bag action");
}

#[test]
fn dragging_listings_into_the_basket_stages_up_to_stock_and_back_removes() {
    let mut app = app(browse_state(vec![buyable(2, 2, 500)]));
    spawn_window(&mut app);
    let window = app.world_mut().spawn(bevy::window::Window::default()).id();
    let listing = source_entity(&mut app, DragKind::Listing, 2);
    let basket = zone_entity(&mut app, DropZone::Basket);

    for _ in 0..3 {
        app.world_mut().trigger(drag_start(listing, window));
        app.world_mut().trigger(drag_drop(basket, listing, window));
    }
    assert_eq!(basket_quantity(&app, 2), 2, "capped at the listing's stock");

    app.update();
    let handle = source_entity(&mut app, DragKind::Basket, 2);
    app.world_mut().trigger(drag_start(handle, window));
    let vendor = zone_entity(&mut app, DropZone::VendorListing);
    app.world_mut().trigger(drag_drop(vendor, handle, window));
    assert_eq!(basket_quantity(&app, 2), 0, "dropped back on the shop");
}

#[test]
fn staging_is_inert_while_a_purchase_is_pending() {
    let mut app = app(browse_state(vec![buyable(2, 3, 500)]));
    spawn_window(&mut app);
    let window = app.world_mut().spawn(bevy::window::Window::default()).id();
    assert!(
        app.world_mut()
            .resource_mut::<VendingState>()
            .begin_purchase(VENDOR_ID, SHOP_ID, 0.0)
            .is_some()
    );
    let listing = source_entity(&mut app, DragKind::Listing, 2);
    let basket = zone_entity(&mut app, DropZone::Basket);

    app.world_mut().trigger(drag_start(listing, window));
    app.world_mut().trigger(drag_drop(basket, listing, window));
    assert_eq!(basket_quantity(&app, 2), 0);
}

#[test]
fn right_click_details_resolve_every_vending_surface() {
    let item_db = item_db();
    let mut cart = cart();
    let mut buckler = cart_item(3, 2104, 1);
    buckler.cards = vec![4001];
    cart.upsert(buckler);
    let source = |kind, index| DragSource { kind, index };

    let seller = VendingState::default();
    for kind in [DragKind::Stock, DragKind::Draft] {
        let view = super::inspect_view(source(kind, 3), &seller, &cart, &item_db, None)
            .expect("cart item details");
        assert_eq!(view.name, "Buckler");
        assert_eq!(view.cards, vec!["Card #4001".to_string()], "{kind:?}");
    }
    assert!(
        super::inspect_view(source(DragKind::Stock, 9), &seller, &cart, &item_db, None).is_none(),
        "an empty cart slot has no details"
    );

    let buyer = browse_state(vec![buyable(2, 3, 500)]);
    for kind in [DragKind::Listing, DragKind::Basket] {
        let view = super::inspect_view(source(kind, 2), &buyer, &cart, &item_db, None)
            .expect("vendor listing details");
        assert_eq!(view.name, "Red Potion");
        assert_eq!(view.meta[0], ("Price".to_string(), "500z".to_string()));
    }

    let mut own = state_with(VendingIngressPayload::SetupReady(setup()));
    let request_id = own.begin_open(SETUP_ID, 0.0).expect("open request");
    own.apply(&VendingIngress {
        generation: ZoneSessionGeneration::default(),
        payload: VendingIngressPayload::OpenResult {
            request_id,
            setup_id: SETUP_ID.into(),
            setup_active: false,
            outcome: VendingOpenOutcome::Success,
            snapshot: Some(snapshot()),
        },
    });
    let view = super::inspect_view(source(DragKind::Shop, 2), &own, &cart, &item_db, None)
        .expect("own shop listing details");
    assert_eq!(view.name, "Red Potion");
    assert!(
        super::inspect_view(source(DragKind::Listing, 2), &own, &cart, &item_db, None).is_none(),
        "the seller is not browsing anyone"
    );
}

#[test]
fn right_clicking_a_stock_cell_opens_its_details_without_actions() {
    let mut app = app(state_with(VendingIngressPayload::SetupReady(setup())));
    spawn_window(&mut app);
    let window = app.world_mut().spawn(bevy::window::Window::default()).id();
    let stock = source_entity(&mut app, DragKind::Stock, 2);

    app.world_mut()
        .trigger(click_event(stock, window, PointerButton::Secondary));
    let written: Vec<super::ShowInfoModal> = app
        .world_mut()
        .resource_mut::<Messages<super::ShowInfoModal>>()
        .drain()
        .collect();
    assert_eq!(written.len(), 1);
    let super::InfoContent::Item { view, action } = &written[0].content else {
        panic!("expected an item view");
    };
    assert_eq!(view.name, "Red Potion");
    assert!(action.is_none(), "cart stock offers no bag action");
    assert!(draft_rows(&app).is_empty(), "inspecting never lists");
}
