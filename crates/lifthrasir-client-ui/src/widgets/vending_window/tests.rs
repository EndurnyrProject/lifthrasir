//! Vending window regressions: draft rules, buttons, bodies, and feedback.

use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_client::domain::cart::Cart;
use lifthrasir_client::domain::entities::registry::EntityRegistry;
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

use super::buyer::BuyerBasket;
use super::feedback::vending_notice_text;
use super::scene;
use super::{
    DraftFieldKind, DraftRow, SellerDraft, SetupReadout, Surface, VendingAction, VendingDraftField,
    VendingItem, VendingOpenButton, VendingQtyField, VendingTitleField, VendingUi,
    VendingWindowRoot, cancel_on_escape, deliver_vending_feedback, focus_new_price_field,
    on_item_inspect, on_vending_button, rebuild_content, sync_setup_readouts, sync_window_state,
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

fn listing(index: u32, amount: u32, price: u32) -> VendingListing {
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

fn snapshot() -> VendingSnapshot {
    VendingSnapshot {
        vendor_unit_id: VENDOR_ID,
        shop_id: SHOP_ID.into(),
        title: "Potions".into(),
        items: vec![listing(2, 2, 500)],
        total_earned: 0,
    }
}

fn ingress(payload: VendingIngressPayload) -> VendingIngress {
    VendingIngress {
        generation: ZoneSessionGeneration::default(),
        payload,
    }
}

fn setup_state() -> VendingState {
    let mut state = VendingState::default();
    state.apply(&ingress(VendingIngressPayload::SetupReady(setup())));
    state
}

/// The seller with an accepted, open shop.
fn open_state() -> VendingState {
    let mut state = setup_state();
    let request_id = state.begin_open(SETUP_ID, 0.0).expect("open request");
    state.apply(&ingress(VendingIngressPayload::OpenResult {
        request_id,
        setup_id: SETUP_ID.into(),
        setup_active: false,
        outcome: VendingOpenOutcome::Success,
        snapshot: Some(snapshot()),
    }));
    state
}

fn browse_state(items: Vec<VendingListing>) -> VendingState {
    let mut state = VendingState::default();
    state.begin_browse(VENDOR_ID).expect("browse request");
    state.apply(&ingress(VendingIngressPayload::ListResult {
        request_id: 1,
        vendor_unit_id: VENDOR_ID,
        outcome: net_contract::dto::VendingListOutcome::Success,
        snapshot: Some(VendingSnapshot {
            items,
            ..snapshot()
        }),
    }));
    state
}

fn draft(rows: &[(u32, u32, Option<u32>)]) -> SellerDraft {
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
        .init_resource::<EntityRegistry>()
        .init_resource::<SellerDraft>()
        .init_resource::<BuyerBasket>()
        .init_resource::<VendingUi>()
        .init_resource::<InputFocus>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_message::<VendingIntent>()
        .add_message::<VendingNotice>()
        .add_message::<RevealVendingSeller>()
        .add_message::<ChatHeard>()
        .add_message::<super::ShowInfoModal>();
    app
}

fn press(app: &mut App, action: VendingAction) {
    let button = app
        .world_mut()
        .spawn(action)
        .observe(on_vending_button)
        .id();
    app.world_mut().trigger(Activate { entity: button });
    app.world_mut().flush();
}

fn intents(app: &mut App) -> Vec<VendingIntent> {
    app.world_mut()
        .resource_mut::<Messages<VendingIntent>>()
        .drain()
        .collect()
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

fn draft_rows(app: &App) -> Vec<DraftRow> {
    app.world().resource::<SellerDraft>().rows.clone()
}

fn basket_quantity(app: &App, index: u32) -> u32 {
    app.world().resource::<BuyerBasket>().quantity(index)
}

/// Spawns the frame and runs the systems that fill and follow it.
fn spawn_window(app: &mut App) {
    app.add_systems(
        Update,
        (
            sync_window_state,
            rebuild_content,
            focus_new_price_field,
            sync_setup_readouts,
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

fn hidden(app: &mut App) -> bool {
    let world = app.world_mut();
    world
        .query_filtered::<&Visibility, With<VendingWindowRoot>>()
        .iter(world)
        .all(|visibility| *visibility == Visibility::Hidden)
}

fn texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut query = world.query::<&Text>();
    query.iter(world).map(|text| text.0.clone()).collect()
}

fn readout(app: &mut App, wanted: SetupReadout) -> String {
    let world = app.world_mut();
    let mut query = world.query::<(&SetupReadout, &Text)>();
    query
        .iter(world)
        .find(|(readout, _)| **readout == wanted)
        .map(|(_, text)| text.0.clone())
        .unwrap_or_else(|| panic!("no {wanted:?} read-out"))
}

fn items(app: &mut App, surface: Surface) -> Vec<Entity> {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &VendingItem)>();
    query
        .iter(world)
        .filter(|(_, item)| item.surface == surface)
        .map(|(entity, _)| entity)
        .collect()
}

fn draft_field(app: &mut App, cart_index: u32, kind: DraftFieldKind) -> Entity {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &VendingDraftField)>();
    query
        .iter(world)
        .find(|(_, field)| field.cart_index == cart_index && field.kind == kind)
        .map(|(entity, _)| entity)
        .unwrap_or_else(|| panic!("no {kind:?} field for slot {cart_index}"))
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

#[test]
fn offers_follow_draft_order_and_leave_out_unpriced_rows() {
    let draft = draft(&[(3, 1, Some(900)), (9, 1, None), (2, 2, Some(250))]);
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
fn add_lists_the_whole_stack_unpriced_once_within_the_slot_cap() {
    let mut draft = SellerDraft::default();
    let live = setup();

    assert_eq!(draft.add(2, 3, &live), Ok(true));
    assert_eq!(
        draft.rows,
        vec![DraftRow {
            cart_index: 2,
            amount: 3,
            price: None
        }],
        "the whole stack, waiting for a price"
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
fn validation_rejects_counts_slots_quantities_and_prices() {
    let cart = cart();
    let live = setup();
    let free = Some(0);

    assert!(draft(&[]).validate(&live, &cart).is_err(), "no listings");

    let mut too_many = draft(&[]);
    too_many.rows = (0..13)
        .map(|index| DraftRow {
            cart_index: index,
            amount: 1,
            price: free,
        })
        .collect();
    assert!(too_many.validate(&live, &cart).is_err(), "over max slots");

    assert!(
        draft(&[(2, 1, free), (2, 1, free)])
            .validate(&live, &cart)
            .is_err(),
        "duplicate slot"
    );
    assert!(
        draft(&[(9, 1, free)]).validate(&live, &cart).is_err(),
        "slot is not sellable"
    );
    assert!(
        draft(&[(2, 0, free)]).validate(&live, &cart).is_err(),
        "zero quantity"
    );
    assert!(
        draft(&[(2, 4, free)]).validate(&live, &cart).is_err(),
        "quantity over stock"
    );
    assert!(
        draft(&[(2, 1, None)]).validate(&live, &cart).is_err(),
        "no price typed"
    );

    let capped = VendingSetup {
        min_unit_price: 100,
        max_unit_price: 5_000,
        ..live.clone()
    };
    let priced = |price| draft(&[(2, 1, Some(price))]).validate(&capped, &cart);
    assert!(priced(99).is_err(), "price below the accepted minimum");
    assert!(priced(5_001).is_err(), "price above the accepted maximum");
    assert!(priced(100).is_ok(), "inclusive minimum");
    assert!(priced(5_000).is_ok(), "inclusive maximum");
    assert!(
        draft(&[(2, 1, free)]).validate(&live, &cart).is_ok(),
        "a typed zero price is allowed by policy"
    );
}

#[test]
fn validation_rejects_empty_control_and_oversized_titles() {
    let cart = cart();
    let mut draft = draft(&[(2, 1, Some(0))]);

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

#[test]
fn listing_a_slot_drafts_it_and_focuses_its_price_and_unlisting_removes_it() {
    let mut app = app(setup_state());
    spawn_window(&mut app);

    press(&mut app, VendingAction::List(2));
    assert_eq!(
        draft_rows(&app),
        vec![DraftRow {
            cart_index: 2,
            amount: 3,
            price: None
        }]
    );
    press(&mut app, VendingAction::List(2));
    assert_eq!(draft_rows(&app).len(), 1, "a slot is listed once");

    app.update();
    let price = draft_field(&mut app, 2, DraftFieldKind::Price);
    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(price),
        "the new row's price takes focus"
    );

    press(&mut app, VendingAction::Unlist(2));
    assert!(draft_rows(&app).is_empty());
}

#[test]
fn a_full_shop_refuses_a_listing_with_a_message() {
    let full = VendingSetup {
        max_slots: 1,
        ..setup()
    };
    let mut state = VendingState::default();
    state.apply(&ingress(VendingIngressPayload::SetupReady(full)));
    let mut app = app(state);
    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, Some(0))]).rows;

    press(&mut app, VendingAction::List(3));
    assert_eq!(draft_rows(&app).len(), 1);
    assert_eq!(
        app.world().resource::<VendingUi>().error(),
        Some("A shop can list at most 1 items.")
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
    let mut app = app(setup_state());
    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, None), (3, 1, None)]).rows;
    spawn_fields(&mut app, " Potions ", &[(2, "3", "450"), (3, "1", "0")]);

    press(&mut app, VendingAction::OpenShop);

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
fn an_invalid_or_unpriced_draft_reports_instead_of_submitting() {
    let mut app = app(setup_state());
    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, Some(0))]).rows;
    spawn_fields(&mut app, "Potions", &[(2, "4", "0")]);
    press(&mut app, VendingAction::OpenShop);
    assert!(intents(&mut app).is_empty());
    assert_eq!(
        app.world().resource::<VendingUi>().error(),
        Some("Quantity must be between 1 and 3.")
    );

    let mut app = self::app(setup_state());
    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, Some(500))]).rows;
    spawn_fields(&mut app, "Potions", &[(2, "1", "")]);
    press(&mut app, VendingAction::OpenShop);
    assert!(intents(&mut app).is_empty(), "an emptied price never opens");
    assert_eq!(
        app.world().resource::<VendingUi>().error(),
        Some("Set a price for every listing.")
    );
}

#[test]
fn submission_waits_for_an_unanswered_open() {
    let mut state = setup_state();
    assert!(state.begin_open(SETUP_ID, 0.0).is_some());
    let mut app = app(state);
    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, Some(0))]).rows;
    spawn_fields(&mut app, "Potions", &[(2, "1", "0")]);

    press(&mut app, VendingAction::OpenShop);
    assert!(intents(&mut app).is_empty());
}

#[test]
fn cancel_close_and_escape_cancel_preparation_exactly_once() {
    let mut app = app(setup_state());
    press(&mut app, VendingAction::CancelSetup);
    assert_eq!(
        intents(&mut app),
        vec![VendingIntent::Close {
            target: VendingCloseTarget::Setup(SETUP_ID.into()),
        }]
    );

    press(&mut app, VendingAction::CloseWindow);
    app.world_mut().resource_mut::<VendingUi>().visible = true;
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
    let mut app = app(open_state());
    app.add_systems(Update, cancel_on_escape);

    app.world_mut().resource_mut::<VendingUi>().visible = true;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert!(!app.world().resource::<VendingUi>().is_visible());

    app.world_mut().resource_mut::<VendingUi>().visible = true;
    press(&mut app, VendingAction::CloseWindow);
    assert!(!app.world().resource::<VendingUi>().is_visible());
    assert!(
        intents(&mut app).is_empty(),
        "hiding must not close the shop"
    );
}

#[test]
fn closing_a_shop_asks_first_and_then_waits_for_the_result() {
    let mut app = app(open_state());

    press(&mut app, VendingAction::AskClose);
    assert!(intents(&mut app).is_empty(), "the prompt sends nothing");
    press(&mut app, VendingAction::KeepOpen);
    assert!(intents(&mut app).is_empty());

    press(&mut app, VendingAction::AskClose);
    press(&mut app, VendingAction::CloseShop);
    assert_eq!(
        intents(&mut app),
        vec![VendingIntent::Close {
            target: VendingCloseTarget::Shop(SHOP_ID.into()),
        }]
    );

    press(&mut app, VendingAction::CloseShop);
    assert!(
        intents(&mut app).is_empty(),
        "a second close waits for the result"
    );
}

#[test]
fn a_settled_close_makes_the_affordance_usable_again() {
    let mut app = app(setup_state());
    app.add_systems(Update, sync_window_state);

    press(&mut app, VendingAction::CancelSetup);
    assert_eq!(intents(&mut app).len(), 1);
    press(&mut app, VendingAction::CancelSetup);
    assert!(
        intents(&mut app).is_empty(),
        "one cancel waits for the result"
    );

    // Nothing is pending in the domain here, so the next frame re-enables it.
    app.update();
    press(&mut app, VendingAction::CancelSetup);
    assert_eq!(intents(&mut app).len(), 1);
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
    let chat = |app: &mut App| -> Vec<String> {
        app.world_mut()
            .resource_mut::<Messages<ChatHeard>>()
            .drain()
            .map(|line| line.message.clone())
            .collect()
    };

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
    assert!(items(&mut idle, Surface::Cart).is_empty());
    assert!(hidden(&mut idle));

    let mut app = app(setup_state());
    spawn_window(&mut app);
    assert!(!hidden(&mut app));
    let shown = texts(&mut app);
    assert!(shown.iter().any(|text| text == "Set Up Shop"), "{shown:?}");
    assert!(shown.iter().any(|text| text == "30 SP \u{b7} Already paid"));
}

#[test]
fn one_stock_row_per_cart_slot_and_one_listing_per_drafted_slot() {
    let mut app = app(setup_state());
    spawn_window(&mut app);
    assert_eq!(items(&mut app, Surface::Cart).len(), 2, "both cart slots");
    assert_eq!(count::<VendingDraftField>(&mut app), 0);

    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, None)]).rows;
    app.update();
    assert_eq!(items(&mut app, Surface::Cart).len(), 3, "and its listing");
    assert_eq!(
        count::<VendingDraftField>(&mut app),
        2,
        "quantity and price"
    );

    app.world_mut().resource_mut::<SellerDraft>().rows = draft(&[(2, 1, None), (3, 1, None)]).rows;
    app.update();
    assert_eq!(count::<VendingDraftField>(&mut app), 4);
}

#[test]
fn typed_fields_survive_unrelated_updates_and_carry_over_a_relisting() {
    let mut app = app(setup_state());
    spawn_window(&mut app);
    press(&mut app, VendingAction::List(2));
    app.update();
    let fields = |app: &mut App| -> Vec<Entity> {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<VendingDraftField>>();
        query.iter(world).collect()
    };
    let before = fields(&mut app);
    assert_eq!(before.len(), 2);

    // An unrelated cart edit must not respawn (and unfocus) the typed fields.
    app.world_mut()
        .resource_mut::<Cart>()
        .upsert(cart_item(7, 501, 9));
    app.update();
    assert_eq!(
        fields(&mut app),
        before,
        "typed fields are kept while the set is stable"
    );

    // Listing another slot respawns the body, which must keep what was typed.
    let price = draft_field(&mut app, 2, DraftFieldKind::Price);
    *app.world_mut().get_mut::<EditableText>(price).unwrap() = EditableText::new("450");
    press(&mut app, VendingAction::List(3));
    app.update();
    assert_eq!(draft_rows(&app)[0].price, Some(450));
    let price = draft_field(&mut app, 2, DraftFieldKind::Price);
    let typed = app.world().get::<EditableText>(price).unwrap();
    assert_eq!(typed.value().to_string(), "450");
}

#[test]
fn setup_readouts_and_the_open_button_follow_what_is_typed() {
    let mut app = app(setup_state());
    spawn_window(&mut app);
    press(&mut app, VendingAction::List(2));
    app.update();
    let open = |app: &mut App| -> bool {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&Pickable, With<VendingOpenButton>>();
        query.single(world).expect("open button").is_hoverable
    };

    assert_eq!(
        readout(&mut app, SetupReadout::Status),
        "Give your shop a name"
    );
    assert_eq!(readout(&mut app, SetupReadout::Note(2)), "Set a price");
    assert_eq!(readout(&mut app, SetupReadout::Sign), "Your shop name");
    assert!(!open(&mut app));

    let title = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<VendingTitleField>>();
        query.single(world).expect("title field")
    };
    *app.world_mut().get_mut::<EditableText>(title).unwrap() = EditableText::new("Potions");
    let price = draft_field(&mut app, 2, DraftFieldKind::Price);
    *app.world_mut().get_mut::<EditableText>(price).unwrap() = EditableText::new("2000000");
    app.update();

    assert_eq!(readout(&mut app, SetupReadout::Status), "Ready to open");
    assert_eq!(readout(&mut app, SetupReadout::Sign), "Potions");
    assert_eq!(readout(&mut app, SetupReadout::TitleCount), "7 / 79");
    assert_eq!(readout(&mut app, SetupReadout::Note(2)), "3 in cart");
    assert_eq!(readout(&mut app, SetupReadout::Words(2)), "2 million");
    assert_eq!(readout(&mut app, SetupReadout::Total(2)), "6,000,000 z");
    assert_eq!(readout(&mut app, SetupReadout::Potential), "6,000,000 z");
    assert!(open(&mut app));
}

#[test]
fn an_accepted_shop_shows_its_stock_and_follows_its_sales() {
    let mut app = app(open_state());
    spawn_window(&mut app);
    app.world_mut().resource_mut::<VendingUi>().visible = true;
    app.update();

    assert_eq!(count::<VendingDraftField>(&mut app), 0);
    assert_eq!(items(&mut app, Surface::Shop).len(), 1);
    let shown = texts(&mut app);
    assert!(shown.iter().any(|text| text == "Your Shop"), "{shown:?}");
    assert!(shown.iter().any(|text| text == "Red Potion"));
    assert!(shown.iter().any(|text| text == "0 of 2 sold"));
    assert!(shown.iter().any(|text| text == "No sales yet"));

    app.world_mut()
        .resource_mut::<VendingState>()
        .apply(&ingress(VendingIngressPayload::SaleReport {
            shop_id: SHOP_ID.into(),
            index: 2,
            nameid: 501,
            amount: 1,
            zeny_gained: 500,
            buyer_name: "Thor".into(),
            sale_sequence: 1,
            total_earned: 500,
        }));
    app.update();
    let shown = texts(&mut app);
    assert!(shown.iter().any(|text| text == "1 of 2 sold"), "{shown:?}");
    assert!(shown.iter().any(|text| text == "Thor"));
    assert!(shown.iter().any(|text| text == "+500 z"));
    assert!(shown.iter().any(|text| text == "1 item"), "one left");
}

#[test]
fn a_closed_shop_shows_its_summary_until_it_is_dismissed() {
    let mut app = app(open_state());
    spawn_window(&mut app);
    let request_id = app
        .world_mut()
        .resource_mut::<VendingState>()
        .begin_close(&VendingCloseTarget::Shop(SHOP_ID.into()), 0.0)
        .expect("close request");
    app.world_mut()
        .resource_mut::<VendingState>()
        .apply(&ingress(VendingIngressPayload::CloseResult {
            request_id,
            target: VendingCloseTarget::Shop(SHOP_ID.into()),
            outcome: VendingCloseOutcome::ShopClosed,
        }));
    app.update();

    assert!(!hidden(&mut app), "the summary opens by itself");
    let shown = texts(&mut app);
    assert!(shown.iter().any(|text| text == "Shop closed."), "{shown:?}");
    assert!(
        shown
            .iter()
            .any(|text| text == "You earned 0 z. 2 unsold items went back to your cart.")
    );

    press(&mut app, VendingAction::SetUpAgain);
    assert_eq!(intents(&mut app), vec![VendingIntent::EnterSeller]);
    app.update();
    app.update();
    assert!(hidden(&mut app), "a dismissed summary stays away");
}

#[test]
fn checkout_emits_one_confirmed_basket_quoted_from_the_shop() {
    let mut app = app(browse_state(vec![listing(2, 3, 500), listing(3, 1, 750)]));
    with_zeny(&mut app, 2_000);
    app.add_systems(Update, super::sync_browse_basket);
    app.update();

    // Row 2 has "2" typed, row 3 has no field and adds one.
    app.world_mut()
        .spawn((VendingQtyField(2), EditableText::new("2")));
    press(&mut app, VendingAction::AddRow(2));
    press(&mut app, VendingAction::AddRow(3));
    let shop = app
        .world()
        .resource::<VendingState>()
        .browse()
        .unwrap()
        .clone();
    assert_eq!(
        app.world().resource::<BuyerBasket>().total(&shop),
        Some(1_750)
    );

    press(&mut app, VendingAction::Buy);
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
    press(&mut app, VendingAction::Buy);
    assert!(intents(&mut app).is_empty());
}

#[test]
fn checkout_refuses_an_empty_basket_and_short_funds() {
    let mut app = app(browse_state(vec![listing(2, 3, 500)]));
    with_zeny(&mut app, 200);
    press(&mut app, VendingAction::Buy);
    assert!(intents(&mut app).is_empty());
    assert_eq!(
        app.world().resource::<VendingUi>().error(),
        Some("Choose at least one item.")
    );

    press(&mut app, VendingAction::AddRow(2));
    press(&mut app, VendingAction::Buy);
    assert!(intents(&mut app).is_empty(), "200z cannot buy a 500z item");
    assert_eq!(
        app.world().resource::<VendingUi>().error(),
        Some("You need 300 more zeny.")
    );
}

#[test]
fn a_large_total_asks_before_it_is_sent() {
    let mut app = app(browse_state(vec![listing(2, 3, 600_000)]));
    with_zeny(&mut app, 5_000_000);
    app.world_mut()
        .spawn((VendingQtyField(2), EditableText::new("2")));
    press(&mut app, VendingAction::AddRow(2));

    press(&mut app, VendingAction::Buy);
    assert!(
        intents(&mut app).is_empty(),
        "1,200,000 z needs a second look"
    );
    assert!(app.world().resource::<VendingUi>().confirm);

    press(&mut app, VendingAction::BackFromConfirm);
    assert!(!app.world().resource::<VendingUi>().confirm);
    press(&mut app, VendingAction::Buy);
    press(&mut app, VendingAction::BuyNow);
    assert_eq!(intents(&mut app).len(), 1);
    assert!(!app.world().resource::<VendingUi>().confirm);
}

#[test]
fn the_row_stepper_and_add_stay_inside_what_the_basket_has_not_taken() {
    let mut app = app(browse_state(vec![listing(2, 3, 500)]));
    let field = app
        .world_mut()
        .spawn((VendingQtyField(2), EditableText::new("1")))
        .id();
    app.update();
    let typed = |app: &App| {
        let text = app.world().get::<EditableText>(field).unwrap();
        text.value().to_string()
    };

    for _ in 0..5 {
        press(&mut app, VendingAction::RowInc(2));
    }
    assert_eq!(typed(&app), "3", "capped at the listing's stock");
    press(&mut app, VendingAction::RowDec(2));
    assert_eq!(typed(&app), "2");
    assert_eq!(basket_quantity(&app, 2), 0, "stepping stages nothing");

    press(&mut app, VendingAction::AddRow(2));
    press(&mut app, VendingAction::AddRow(2));
    assert_eq!(basket_quantity(&app, 2), 3, "never more than is for sale");

    press(&mut app, VendingAction::RemoveLine(2));
    assert_eq!(basket_quantity(&app, 2), 0);
}

#[test]
fn staging_is_inert_while_a_purchase_is_pending() {
    let mut app = app(browse_state(vec![listing(2, 3, 500)]));
    assert!(
        app.world_mut()
            .resource_mut::<VendingState>()
            .begin_purchase(VENDOR_ID, SHOP_ID, 0.0)
            .is_some()
    );

    press(&mut app, VendingAction::AddRow(2));
    assert_eq!(basket_quantity(&app, 2), 0);
}

#[test]
fn a_settled_success_clears_the_basket_and_a_failure_keeps_it() {
    let mut app = app(browse_state(vec![listing(2, 3, 500)]));
    app.add_systems(
        Update,
        (super::sync_browse_basket, super::settle_buyer_basket).chain(),
    );
    app.update();
    press(&mut app, VendingAction::AddRow(2));
    assert_eq!(basket_quantity(&app, 2), 1);

    let settle = |app: &mut App, request_id, outcome| {
        app.world_mut()
            .resource_mut::<Messages<VendingNotice>>()
            .write(VendingNotice::Purchase {
                request_id,
                vendor_unit_id: VENDOR_ID,
                shop_id: SHOP_ID.into(),
                outcome,
            });
        app.update();
    };
    settle(&mut app, 4, VendingPurchaseOutcome::NotEnoughZeny);
    assert_eq!(
        basket_quantity(&app, 2),
        1,
        "a failed checkout keeps the basket"
    );

    settle(&mut app, 5, VendingPurchaseOutcome::Success);
    assert!(
        app.world().resource::<BuyerBasket>().is_empty(),
        "a completed checkout clears the basket"
    );
}

#[test]
fn a_refreshed_shop_clamps_the_basket_and_explains_it() {
    let mut app = app(browse_state(vec![listing(2, 3, 500)]));
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
        .resource_mut::<BuyerBasket>()
        .set_quantity(&shop, 2, 3);

    let refresh = app
        .world_mut()
        .resource_mut::<VendingState>()
        .begin_browse(VENDOR_ID)
        .expect("refresh request");
    app.world_mut()
        .resource_mut::<VendingState>()
        .apply(&ingress(VendingIngressPayload::ListResult {
            request_id: refresh,
            vendor_unit_id: VENDOR_ID,
            outcome: net_contract::dto::VendingListOutcome::Success,
            snapshot: Some(VendingSnapshot {
                items: vec![listing(2, 1, 500)],
                ..snapshot()
            }),
        }));
    app.update();

    assert_eq!(basket_quantity(&app, 2), 1, "reduced stock is clamped");
    assert_eq!(
        app.world().resource::<VendingUi>().change.as_deref(),
        Some("Only 1 of Item #501 remain.")
    );
    press(&mut app, VendingAction::AckChange);
    assert_eq!(app.world().resource::<VendingUi>().change, None);
}

#[test]
fn a_browsed_shop_lists_its_stock_beside_the_basket() {
    let mut app = app(browse_state(vec![listing(2, 3, 500), listing(3, 0, 750)]));
    with_zeny(&mut app, 2_000);
    spawn_window(&mut app);
    app.world_mut().resource_mut::<VendingUi>().visible = true;
    app.update();

    assert_eq!(items(&mut app, Surface::Listing).len(), 2);
    assert_eq!(
        count::<VendingQtyField>(&mut app),
        1,
        "sold-out rows have none"
    );
    let shown = texts(&mut app);
    assert!(shown.iter().any(|text| text == "Shop"), "{shown:?}");
    assert!(shown.iter().any(|text| text == "1 listing"));
    assert!(shown.iter().any(|text| text == "Sold out"));
    assert!(shown.iter().any(|text| text == "Buy"));

    press(&mut app, VendingAction::AddRow(2));
    app.update();
    assert_eq!(
        items(&mut app, Surface::Listing).len(),
        3,
        "and a basket line"
    );
    let shown = texts(&mut app);
    assert!(shown.iter().any(|text| text == "1 in cart"), "{shown:?}");
    assert!(shown.iter().any(|text| text == "Buy for 500 z"));
    assert!(shown.iter().any(|text| text == "1,500 z"), "zeny after");
}

#[test]
fn right_click_details_resolve_every_vending_surface() {
    let item_db = item_db();
    let mut cart = cart();
    let mut buckler = cart_item(3, 2104, 1);
    buckler.cards = vec![4001];
    cart.upsert(buckler);
    let item = |surface, index| VendingItem { surface, index };

    let seller = VendingState::default();
    let view = super::inspect_view(item(Surface::Cart, 3), &seller, &cart, &item_db, None)
        .expect("cart item details");
    assert_eq!(view.name, "Buckler");
    assert_eq!(view.cards, vec!["Card #4001".to_string()]);
    assert!(
        super::inspect_view(item(Surface::Cart, 9), &seller, &cart, &item_db, None).is_none(),
        "an empty cart slot has no details"
    );

    let buyer = browse_state(vec![listing(2, 3, 500)]);
    let view = super::inspect_view(item(Surface::Listing, 2), &buyer, &cart, &item_db, None)
        .expect("vendor listing details");
    assert_eq!(view.name, "Red Potion");
    assert_eq!(view.meta[0], ("Price".to_string(), "500z".to_string()));

    let own = open_state();
    let view = super::inspect_view(item(Surface::Shop, 2), &own, &cart, &item_db, None)
        .expect("own shop listing details");
    assert_eq!(view.name, "Red Potion");
    assert!(
        super::inspect_view(item(Surface::Listing, 2), &own, &cart, &item_db, None).is_none(),
        "the seller is not browsing anyone"
    );
}

#[test]
fn right_clicking_a_row_opens_its_details_without_actions_or_side_effects() {
    let mut app = app(setup_state());
    let window = app.world_mut().spawn(bevy::window::Window::default()).id();
    let row = app
        .world_mut()
        .spawn(VendingItem {
            surface: Surface::Cart,
            index: 2,
        })
        .observe(on_item_inspect)
        .id();

    app.world_mut()
        .trigger(click_event(row, window, PointerButton::Primary));
    assert!(
        app.world()
            .resource::<Messages<super::ShowInfoModal>>()
            .is_empty(),
        "a primary click inspects nothing"
    );

    app.world_mut()
        .trigger(click_event(row, window, PointerButton::Secondary));
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
