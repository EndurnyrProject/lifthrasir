use bevy::ecs::system::RunSystemOnce;
use bevy::scene::ScenePlugin;
use lifthrasir_client::domain::inventory::Item;
use lifthrasir_data::{ItemData, ItemInfo};
use net_contract::dto::StorageItem;

use super::view::{MetaLine, Sources};
use super::*;

const POTION: u32 = 501;
const JELLOPY: u32 = 909;
const FEATHER: u32 = 949;
const BUCKLER: u32 = 2104;

fn item_db() -> ItemDb {
    let mut data = ItemData::default();
    for (id, name) in [
        (POTION, "Red Potion"),
        (JELLOPY, "Jellopy"),
        (FEATHER, "Feather"),
        (BUCKLER, "Buckler"),
    ] {
        data.items.insert(
            id,
            ItemInfo {
                identified_name: name.to_string(),
                identified_resource: name.to_uppercase().replace(' ', "_"),
                ..Default::default()
            },
        );
    }
    ItemDb::from_item_data(data)
}

/// A bag item. `item_type` is the `IT_*` type: 0 usable, 3 etc, 4 armor.
fn bag(index: u16, item_id: u32, item_type: u8, amount: u16) -> Item {
    Item {
        index,
        item_id,
        item_type,
        amount,
        identified: true,
        ..Default::default()
    }
}

fn stored(index: u32, nameid: u32, type_: u32, amount: u32) -> StorageItem {
    StorageItem {
        index,
        nameid,
        amount,
        type_,
        location: 0,
        attribute: 0,
        refine: 0,
        expire_time: 0,
        look: 0,
        weight: 10,
        identified: true,
        cards: vec![],
        options: vec![],
    }
}

fn inventory_of(items: impl IntoIterator<Item = Item>) -> Inventory {
    let mut inventory = Inventory::default();
    for item in items {
        inventory.upsert(item);
    }
    inventory
}

fn storage_of(kind: StorageKind, items: Vec<StorageItem>) -> Storage {
    let mut storage = Storage::default();
    storage.open(kind, 600, items);
    storage
}

/// A run whose only transfer is waiting for the server's answer.
pub(super) fn flying(source: StorageSelection, amount: u32) -> Run {
    Run {
        kind: RunKind::Loot,
        queue: VecDeque::new(),
        in_flight: Some(Transfer { source, amount }),
        landed: None,
        done: Vec::new(),
        failed: None,
    }
}

/// An app with the containers, the storage messages and the transfer driver. The
/// vault starts closed.
pub(super) fn wire_app() -> App {
    let mut app = App::new();
    app.add_message::<DepositStorageItem>()
        .add_message::<WithdrawStorageItem>()
        .add_message::<CloseStorage>()
        .add_message::<StorageResult>()
        .add_message::<StorageItemAdded>()
        .add_message::<ItemAdded>()
        .add_message::<ShowInfoModal>()
        .init_resource::<Time>()
        .init_resource::<Inventory>()
        .init_resource::<Storage>()
        .init_resource::<StorageUi>()
        .insert_resource(item_db())
        .add_systems(Update, drive_transfers);
    app
}

fn open_app(kind: StorageKind, bag: Vec<Item>, vault: Vec<StorageItem>) -> App {
    let mut app = wire_app();
    *app.world_mut().resource_mut::<Inventory>() = inventory_of(bag);
    *app.world_mut().resource_mut::<Storage>() = storage_of(kind, vault);
    app
}

fn act(app: &mut App, action: StorageAction) {
    app.world_mut()
        .run_system_once(move |mut desk: StorageDesk| desk.act(action))
        .unwrap();
}

fn ui(app: &App) -> &StorageUi {
    app.world().resource::<StorageUi>()
}

/// The `(inventory index, amount)` of every deposit sent since the last call.
fn deposits(app: &mut App) -> Vec<(u32, u32)> {
    app.world_mut()
        .resource_mut::<Messages<DepositStorageItem>>()
        .drain()
        .map(|sent| (sent.inventory_index, sent.amount))
        .collect()
}

/// The `(storage index, amount)` of every withdrawal sent since the last call.
fn withdrawals(app: &mut App) -> Vec<(u32, u32)> {
    app.world_mut()
        .resource_mut::<Messages<WithdrawStorageItem>>()
        .drain()
        .map(|sent| (sent.storage_index, sent.amount))
        .collect()
}

fn answer(app: &mut App, outcome: Result<(), StorageRejection>) {
    app.world_mut().write_message(StorageResult { outcome });
    app.update();
}

/// The server accepts the deposit in flight into vault slot `item.index`.
fn accept_into_vault(app: &mut App, item: StorageItem) {
    let kind = app.world().resource::<Storage>().kind().unwrap();
    app.world_mut()
        .resource_mut::<Storage>()
        .upsert(item.clone());
    app.world_mut()
        .write_message(StorageItemAdded { kind, item });
    answer(app, Ok(()));
}

/// The server accepts the withdrawal in flight into bag slot `index`.
fn accept_into_bag(app: &mut App, index: u32) {
    app.world_mut().write_message(ItemAdded {
        index,
        amount: 1,
        nameid: JELLOPY,
        identified: true,
        attribute: 0,
        refine: 0,
        cards: vec![],
        options: vec![],
        location: 0,
        type_: 3,
        result: 0,
        expire_time: 0,
        look: 0,
    });
    answer(app, Ok(()));
}

fn toast(app: &App) -> (&str, bool) {
    let toast = ui(app).toast.as_ref().expect("a toast is up");
    (toast.text.as_str(), toast.bad)
}

#[test]
fn search_query_is_trimmed_and_case_normalized() {
    let mut ui = StorageUi::default();

    ui.set_query("  Red POTION  ");

    assert_eq!(ui.query(), "red potion");
}

#[test]
fn category_and_search_share_one_case_insensitive_filter() {
    use lifthrasir_client::domain::inventory::ItemCategory;

    let passes =
        |category, query| view::matches_filter(category, query, ItemCategory::Use, "Red Potion");
    assert!(passes(StorageCategory::All, "red"));
    assert!(passes(StorageCategory::Use, ""));
    assert!(!passes(StorageCategory::Etc, "red"));
    assert!(!passes(StorageCategory::Use, "blue"));
}

#[test]
fn the_bag_lists_unworn_items_by_type_then_name() {
    let db = item_db();
    let inventory = inventory_of([
        bag(2, JELLOPY, 3, 5),
        bag(3, BUCKLER, 4, 1),
        bag(4, POTION, 0, 9),
        bag(5, FEATHER, 3, 2),
        Item {
            wear_state: 32,
            ..bag(6, BUCKLER, 4, 1)
        },
    ]);
    let mut ui = StorageUi::default();
    let order = |ui: &StorageUi| -> Vec<u16> {
        view::bag_items(&inventory, Some(&db), ui)
            .into_iter()
            .map(|item| item.index)
            .collect()
    };

    assert_eq!(order(&ui), [4, 3, 5, 2], "use, equip, then etc by name");

    ui.category = StorageCategory::Etc;
    ui.set_query("jel");
    assert_eq!(order(&ui), [2]);
}

#[test]
fn the_vault_follows_the_chosen_order() {
    let db = item_db();
    let storage = storage_of(
        StorageKind::Personal,
        vec![
            stored(1, JELLOPY, 3, 40),
            stored(2, BUCKLER, 4, 1),
            stored(3, POTION, 0, 300),
            stored(4, FEATHER, 3, 7),
        ],
    );
    let mut ui = StorageUi::default();
    let order = |sort: StorageSort, ui: &mut StorageUi| -> Vec<u32> {
        ui.sort = sort;
        view::vault_items(&storage, Some(&db), ui)
            .into_iter()
            .map(|item| item.index)
            .collect()
    };

    assert_eq!(order(StorageSort::Type, &mut ui), [3, 2, 4, 1]);
    assert_eq!(order(StorageSort::Name, &mut ui), [2, 4, 1, 3]);
    assert_eq!(order(StorageSort::Amount, &mut ui), [3, 1, 4, 2]);

    ui.note_stored(4);
    ui.note_stored(2);
    ui.note_stored(4);
    assert_eq!(
        order(StorageSort::Recent, &mut ui),
        [4, 2, 1, 3],
        "newest first, untouched slots after in slot order"
    );
}

#[test]
fn bulk_deposits_leave_favorites_and_worn_items_in_the_bag() {
    let inventory = inventory_of([
        bag(2, JELLOPY, 3, 5),
        Item {
            favorite: true,
            ..bag(3, FEATHER, 3, 2)
        },
        bag(4, POTION, 0, 9),
        bag(5, BUCKLER, 4, 1),
        Item {
            wear_state: 32,
            ..bag(6, BUCKLER, 4, 1)
        },
    ]);
    let storage = storage_of(
        StorageKind::Personal,
        vec![
            stored(1, POTION, 0, 300),
            stored(2, FEATHER, 3, 7),
            stored(3, BUCKLER, 4, 1),
        ],
    );
    let slots = |moves: Vec<Transfer>| -> Vec<(StorageSelection, u32)> {
        moves
            .into_iter()
            .map(|moved| (moved.source, moved.amount))
            .collect()
    };

    assert_eq!(
        slots(view::loot(&inventory)),
        [(StorageSelection::Bag(2), 5)],
        "every plain Etc stack, whole"
    );
    assert_eq!(
        slots(view::top_up(&inventory, &storage)),
        [(StorageSelection::Bag(4), 9)],
        "stacks the vault already holds; equipment never stacks"
    );
}

#[test]
fn the_meta_line_tells_the_weight_after_a_withdrawal_or_that_it_will_not_fit() {
    let bag_item = MetaLine {
        place: "In your bag",
        stack: 40,
        unit_weight: None,
        carried: (100, 2800),
    };
    assert_eq!(
        bag_item.line(40),
        ("In your bag \u{b7} 40".to_string(), ro_dark::MUTED)
    );

    let vault_item = MetaLine {
        place: "In your vault",
        stack: 300,
        unit_weight: Some(10),
        carried: (2700, 2800),
    };
    assert_eq!(
        vault_item.line(5),
        (
            "In your vault \u{b7} 300 \u{b7} Weight 10 each \u{b7} Bag after: 2,750 / 2,800"
                .to_string(),
            ro_dark::MUTED
        )
    );
    assert_eq!(
        vault_item.line(11),
        (
            "Too heavy \u{b7} You can carry 10 more".to_string(),
            ro_dark::STATUS_FULL
        )
    );
}

#[test]
fn chips_and_the_slider_stay_within_the_stack() {
    let picks = |stack| Preset::ALL.map(|preset| preset.of(stack));
    assert_eq!(picks(40), [1, 10, 20, 40]);
    assert_eq!(picks(3), [1, 3, 1, 3]);

    assert_eq!(view::slider_amount(-0.2, 40), 1);
    assert_eq!(view::slider_amount(0.5, 41), 21);
    assert_eq!(view::slider_amount(1.4, 40), 40);
    assert_eq!(view::slider_percent(1, 40), 0.0);
    assert_eq!(view::slider_percent(40, 40), 100.0);
    assert_eq!(view::slider_percent(1, 1), 100.0);
}

#[test]
fn every_storage_rejection_has_a_message() {
    let rejections = [
        StorageRejection::Full,
        StorageRejection::InventoryFull,
        StorageRejection::Overweight,
        StorageRejection::NotStorable,
        StorageRejection::ItemEquipped,
        StorageRejection::InvalidAmount,
        StorageRejection::NotOpen,
        StorageRejection::BasicSkillRequired,
        StorageRejection::NoGuild,
        StorageRejection::GuildNoSkill,
        StorageRejection::GuildNoPermission,
        StorageRejection::GuildInUse,
        StorageRejection::OtherStorageOpen,
        StorageRejection::Rental,
        StorageRejection::NoGuildStorage,
        StorageRejection::Stale,
    ];
    for rejection in rejections {
        assert!(!rejection_message(rejection).is_empty());
    }
    assert_eq!(
        rejection_message(StorageRejection::Unknown(-37)),
        "Storage request failed (code -37)."
    );
}

#[test]
fn the_guild_vault_is_captioned_with_the_guild_and_the_personal_one_is_not() {
    let inventory = inventory_of([bag(2, JELLOPY, 3, 5)]);
    let ui = StorageUi::default();
    let caption = |kind, guild| {
        let storage = storage_of(kind, vec![stored(1, POTION, 0, 300)]);
        let sources = Sources {
            inventory: &inventory,
            storage: &storage,
            item_db: None,
            guild,
            weight: (100, 2800),
            zeny: 0,
        };
        let body = view::body_view(&sources, &ui);
        (body.vault.title, body.vault.count, body.vault.hint.0)
    };

    assert_eq!(
        caption(StorageKind::Personal, Some("Wardens")),
        (
            "VAULT",
            "1 / 600".to_string(),
            "Personal \u{b7} Shared by all your heroes".to_string()
        )
    );
    assert_eq!(
        caption(StorageKind::Guild, Some("Wardens")).2,
        "Shared with Wardens \u{b7} Withdrawals are logged"
    );
}

#[test]
fn committing_moves_the_chosen_amount_to_the_open_vault() {
    for kind in [StorageKind::Personal, StorageKind::Guild] {
        let mut app = open_app(
            kind,
            vec![bag(7, POTION, 0, 40)],
            vec![stored(70_000, JELLOPY, 3, 9)],
        );
        {
            let mut ui = app.world_mut().resource_mut::<StorageUi>();
            ui.selected = Some(StorageSelection::Bag(7));
            ui.qty = 12;
        }
        act(&mut app, StorageAction::Commit);
        app.update();

        let sent: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<DepositStorageItem>>()
            .drain()
            .collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(
            (sent[0].kind, sent[0].inventory_index, sent[0].amount),
            (kind, 7, 12)
        );
        assert_eq!(
            app.world().resource::<Inventory>().get(7).unwrap().amount,
            40,
            "the server moves the items, not the window"
        );

        accept_into_vault(&mut app, stored(70_001, POTION, 0, 12));
        assert_eq!(toast(&app), ("Deposited 12 Red Potion", false));

        {
            let mut ui = app.world_mut().resource_mut::<StorageUi>();
            ui.selected = Some(StorageSelection::Vault(70_000));
            ui.qty = 4;
        }
        act(&mut app, StorageAction::Commit);
        app.update();
        let sent: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<WithdrawStorageItem>>()
            .drain()
            .collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(
            (sent[0].kind, sent[0].storage_index, sent[0].amount),
            (kind, 70_000, 4)
        );
    }
}

#[test]
fn a_bulk_deposit_sends_one_stack_at_a_time_and_undo_sends_them_back() {
    let mut app = open_app(
        StorageKind::Personal,
        vec![
            bag(2, JELLOPY, 3, 5),
            bag(3, FEATHER, 3, 7),
            bag(4, POTION, 0, 9),
        ],
        vec![],
    );

    act(&mut app, StorageAction::DepositLoot);
    app.update();
    assert_eq!(deposits(&mut app), [(2, 5)]);
    app.update();
    assert!(
        deposits(&mut app).is_empty(),
        "the next stack waits for the answer"
    );

    accept_into_vault(&mut app, stored(70, JELLOPY, 3, 5));
    assert_eq!(deposits(&mut app), [(3, 7)]);
    assert!(ui(&app).busy());

    accept_into_vault(&mut app, stored(71, FEATHER, 3, 7));
    assert!(deposits(&mut app).is_empty(), "the potion is not loot");
    assert!(!ui(&app).busy());
    assert_eq!(toast(&app), ("Deposited loot \u{b7} 2 stacks", false));
    assert_eq!(
        ui(&app).last_action(),
        Some("Deposited loot \u{b7} 2 stacks")
    );

    act(&mut app, StorageAction::Undo);
    app.update();
    assert_eq!(
        withdrawals(&mut app),
        [(71, 7)],
        "newest first, from the slot it landed in"
    );
    accept_into_bag(&mut app, 3);
    assert_eq!(withdrawals(&mut app), [(70, 5)]);
    accept_into_bag(&mut app, 2);

    assert_eq!(
        toast(&app),
        ("Undid: Deposited loot \u{b7} 2 stacks", false)
    );
    assert_eq!(ui(&app).last_action(), None, "an undo is not undoable");
}

#[test]
fn undoing_a_withdrawal_deposits_from_the_bag_slot_it_landed_in() {
    let mut app = open_app(
        StorageKind::Personal,
        vec![bag(9, JELLOPY, 3, 4)],
        vec![stored(70, JELLOPY, 3, 40)],
    );
    {
        let mut ui = app.world_mut().resource_mut::<StorageUi>();
        ui.selected = Some(StorageSelection::Vault(70));
        ui.qty = 4;
    }
    act(&mut app, StorageAction::Commit);
    app.update();
    assert_eq!(withdrawals(&mut app), [(70, 4)]);
    accept_into_bag(&mut app, 9);
    assert_eq!(toast(&app), ("Withdrew 4 Jellopy", false));

    act(&mut app, StorageAction::Undo);
    app.update();

    assert_eq!(deposits(&mut app), [(9, 4)]);
}

#[test]
fn a_rejected_item_is_skipped_and_a_full_vault_stops_the_queue() {
    let mut app = open_app(
        StorageKind::Personal,
        vec![
            bag(2, JELLOPY, 3, 5),
            bag(3, FEATHER, 3, 7),
            bag(4, JELLOPY, 3, 1),
            bag(5, FEATHER, 3, 1),
        ],
        vec![],
    );
    act(&mut app, StorageAction::DepositLoot);
    app.update();
    assert_eq!(deposits(&mut app), [(2, 5)]);

    answer(&mut app, Err(StorageRejection::NotStorable));
    assert_eq!(deposits(&mut app), [(3, 7)], "only that item is skipped");

    accept_into_vault(&mut app, stored(70, FEATHER, 3, 7));
    assert_eq!(deposits(&mut app), [(4, 1)]);

    answer(&mut app, Err(StorageRejection::Full));
    assert!(deposits(&mut app).is_empty(), "nothing else would fit");
    assert!(!ui(&app).busy());
    assert_eq!(
        toast(&app),
        (
            "Deposited loot \u{b7} 1 stack \u{b7} then: Storage is full.",
            true
        )
    );
    assert_eq!(ui(&app).history[0].moves.len(), 1);
}

#[test]
fn a_rejected_single_move_says_why_and_leaves_nothing_to_undo() {
    let mut app = open_app(
        StorageKind::Personal,
        vec![],
        vec![stored(70, JELLOPY, 3, 40)],
    );
    {
        let mut ui = app.world_mut().resource_mut::<StorageUi>();
        ui.selected = Some(StorageSelection::Vault(70));
        ui.qty = 40;
    }
    act(&mut app, StorageAction::Commit);
    app.update();
    act(&mut app, StorageAction::Commit);
    app.update();
    assert_eq!(
        withdrawals(&mut app),
        [(70, 40)],
        "a second commit is ignored while the first is in progress"
    );

    answer(&mut app, Err(StorageRejection::Overweight));

    assert_eq!(toast(&app), ("You are carrying too much weight.", true));
    assert_eq!(ui(&app).last_action(), None);
    assert!(!ui(&app).busy());
}

#[test]
fn a_queued_slot_that_vanished_is_skipped_and_an_amount_is_capped_to_the_stack() {
    let mut app = open_app(
        StorageKind::Personal,
        vec![bag(2, JELLOPY, 3, 5), bag(3, FEATHER, 3, 7)],
        vec![],
    );
    let moves = [(2, 5), (9, 1), (3, 99)]
        .map(|(index, amount)| Transfer {
            source: StorageSelection::Bag(index),
            amount,
        })
        .to_vec();
    app.world_mut()
        .resource_mut::<StorageUi>()
        .begin(RunKind::Loot, moves);
    app.update();
    assert_eq!(deposits(&mut app), [(2, 5)]);

    accept_into_vault(&mut app, stored(70, JELLOPY, 3, 5));

    assert_eq!(deposits(&mut app), [(3, 7)]);
}

#[test]
fn an_answer_that_arrives_while_closed_cannot_settle_a_later_transfer() {
    let mut app = wire_app();
    answer(&mut app, Err(StorageRejection::Full));

    *app.world_mut().resource_mut::<Inventory>() = inventory_of([bag(2, JELLOPY, 3, 5)]);
    *app.world_mut().resource_mut::<Storage>() = storage_of(StorageKind::Personal, vec![]);
    act(&mut app, StorageAction::DepositLoot);
    app.update();
    app.update();

    assert_eq!(deposits(&mut app), [(2, 5)]);
    assert!(ui(&app).in_flight().is_some());
    assert_eq!(ui(&app).toast, None);
}

#[test]
fn steps_and_chips_keep_the_amount_within_the_selected_stack() {
    let mut app = open_app(StorageKind::Personal, vec![bag(7, POTION, 0, 40)], vec![]);
    {
        let mut ui = app.world_mut().resource_mut::<StorageUi>();
        ui.selected = Some(StorageSelection::Bag(7));
        ui.qty = 40;
    }
    let mut qty_after = |action| {
        act(&mut app, action);
        ui(&app).qty
    };

    assert_eq!(qty_after(StorageAction::Step(true)), 40);
    assert_eq!(qty_after(StorageAction::Step(false)), 39);
    assert_eq!(qty_after(StorageAction::Amount(Preset::Half)), 20);
    assert_eq!(qty_after(StorageAction::Amount(Preset::One)), 1);
    assert_eq!(qty_after(StorageAction::Step(false)), 1);
}

#[test]
fn the_sort_box_cycles_through_every_order() {
    let mut app = open_app(StorageKind::Personal, vec![], vec![]);
    let mut seen = vec![ui(&app).sort];
    for _ in 0..4 {
        act(&mut app, StorageAction::CycleSort);
        seen.push(ui(&app).sort);
    }

    assert_eq!(
        seen,
        [
            StorageSort::Type,
            StorageSort::Name,
            StorageSort::Amount,
            StorageSort::Recent,
            StorageSort::Type,
        ]
    );
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
        duration: Duration::ZERO,
        count: 1,
    }
}

fn click(app: &mut App, cell: Entity, button: PointerButton) {
    let window = app.world_mut().spawn_empty().id();
    app.world_mut().trigger(click_event(cell, window, button));
}

fn shown_items(app: &mut App) -> usize {
    app.world_mut()
        .resource_mut::<Messages<ShowInfoModal>>()
        .drain()
        .count()
}

#[test]
fn a_click_selects_the_whole_stack_and_a_double_click_moves_it() {
    let mut app = open_app(
        StorageKind::Personal,
        vec![bag(7, POTION, 0, 40), bag(8, JELLOPY, 3, 3)],
        vec![],
    );
    let potion = app
        .world_mut()
        .spawn(StorageCell(StorageSelection::Bag(7)))
        .observe(on_cell_click)
        .id();
    let jellopy = app
        .world_mut()
        .spawn(StorageCell(StorageSelection::Bag(8)))
        .observe(on_cell_click)
        .id();

    click(&mut app, potion, PointerButton::Primary);
    assert_eq!(ui(&app).selected, Some(StorageSelection::Bag(7)));
    assert_eq!(ui(&app).qty, 40);

    click(&mut app, jellopy, PointerButton::Primary);
    app.update();
    assert_eq!(ui(&app).qty, 3);
    assert!(
        deposits(&mut app).is_empty(),
        "two clicks on different slots are not a double-click"
    );

    click(&mut app, jellopy, PointerButton::Primary);
    app.update();
    assert_eq!(deposits(&mut app), [(8, 3)]);
    assert_eq!(shown_items(&mut app), 0);
}

#[test]
fn a_slow_second_click_only_selects() {
    let mut app = open_app(StorageKind::Personal, vec![bag(7, POTION, 0, 40)], vec![]);
    let potion = app
        .world_mut()
        .spawn(StorageCell(StorageSelection::Bag(7)))
        .observe(on_cell_click)
        .id();

    click(&mut app, potion, PointerButton::Primary);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(DOUBLE_CLICK + Duration::from_millis(1));
    click(&mut app, potion, PointerButton::Primary);
    app.update();

    assert!(deposits(&mut app).is_empty());
}

#[test]
fn a_secondary_click_inspects_without_selecting() {
    let mut app = open_app(
        StorageKind::Personal,
        vec![bag(7, POTION, 0, 40)],
        vec![stored(70, JELLOPY, 3, 9)],
    );
    for source in [StorageSelection::Bag(7), StorageSelection::Vault(70)] {
        let cell = app
            .world_mut()
            .spawn(StorageCell(source))
            .observe(on_cell_click)
            .id();
        click(&mut app, cell, PointerButton::Secondary);
        assert_eq!(shown_items(&mut app), 1);
    }

    assert_eq!(ui(&app).selected, None);
}

#[test]
fn a_dropped_stack_asks_how_many_and_a_dropped_single_item_moves_at_once() {
    let mut app = open_app(
        StorageKind::Personal,
        vec![bag(7, POTION, 0, 40), bag(8, BUCKLER, 4, 1)],
        vec![],
    );
    let drop_across = |app: &mut App, source| {
        app.world_mut()
            .run_system_once(move |mut desk: StorageDesk| desk.drop_across(source))
            .unwrap();
        app.update();
    };

    drop_across(&mut app, StorageSelection::Bag(7));
    assert!(ui(&app).asking);
    assert_eq!(ui(&app).selected, Some(StorageSelection::Bag(7)));
    assert_eq!(ui(&app).qty, 40);
    assert!(deposits(&mut app).is_empty());

    act(&mut app, StorageAction::Commit);
    app.update();
    assert!(!ui(&app).asking);
    assert_eq!(deposits(&mut app), [(7, 40)]);
    answer(&mut app, Ok(()));

    drop_across(&mut app, StorageSelection::Bag(8));
    assert_eq!(deposits(&mut app), [(8, 1)]);
}

#[test]
fn the_close_button_asks_the_server_to_end_the_session() {
    for kind in [StorageKind::Personal, StorageKind::Guild] {
        let mut app = open_app(kind, vec![], vec![]);
        let close = app.world_mut().spawn_empty().observe(on_storage_close).id();

        app.world_mut().trigger(Activate { entity: close });

        let sent: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<CloseStorage>>()
            .drain()
            .collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].kind, kind);
    }
}

/// An app that spawns the window and runs its systems, with a local player.
fn window_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<InputFocus>()
        .init_resource::<Inventory>()
        .init_resource::<Storage>()
        .init_resource::<StorageUi>()
        .init_resource::<GuildState>()
        .insert_resource(item_db())
        .add_systems(
            Update,
            (sync_window, sync_search, rebuild, sync_amount, show_toast).chain(),
        );
    app.world_mut().spawn((
        LocalPlayer,
        CharacterStatus {
            weight: 100,
            max_weight: 2800,
            zeny: 1_284_440,
            ..Default::default()
        },
    ));
    app.world_mut()
        .spawn_scene(scene::window())
        .expect("window spawns");
    app.update();
    app
}

fn open_window(app: &mut App, bag: Vec<Item>, vault: Vec<StorageItem>) {
    *app.world_mut().resource_mut::<Inventory>() = inventory_of(bag);
    *app.world_mut().resource_mut::<Storage>() = storage_of(StorageKind::Personal, vault);
    app.update();
    app.update();
}

fn texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut query = world.query::<&Text>();
    query.iter(world).map(|text| text.0.clone()).collect()
}

fn count<F: bevy::ecs::query::QueryFilter>(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut query = world.query_filtered::<(), F>();
    query.iter(world).count()
}

fn root_visibility(app: &mut App) -> Visibility {
    let world = app.world_mut();
    let mut query = world.query_filtered::<&Visibility, With<StorageWindowRoot>>();
    *query.single(world).unwrap()
}

#[test]
fn the_window_shows_only_while_the_vault_is_open_and_starts_over_each_time() {
    let mut app = window_app();
    assert_eq!(root_visibility(&mut app), Visibility::Hidden);
    assert_eq!(count::<With<StorageHost>>(&mut app), 2);

    open_window(&mut app, vec![bag(2, JELLOPY, 3, 5)], vec![]);
    assert_eq!(root_visibility(&mut app), Visibility::Inherited);

    {
        let mut ui = app.world_mut().resource_mut::<StorageUi>();
        ui.category = StorageCategory::Etc;
        ui.selected = Some(StorageSelection::Bag(2));
        ui.run = Some(flying(StorageSelection::Bag(2), 5));
    }
    app.world_mut().resource_mut::<Storage>().close();
    app.update();

    assert_eq!(root_visibility(&mut app), Visibility::Hidden);
    assert_eq!(ui(&app), &StorageUi::default());
}

#[test]
fn the_body_draws_both_panes_in_whole_rows_with_their_captions() {
    let mut app = window_app();
    open_window(
        &mut app,
        vec![bag(2, JELLOPY, 3, 5), bag(4, POTION, 0, 9)],
        vec![stored(70, POTION, 0, 300)],
    );

    let shown = texts(&mut app);
    for expected in [
        "BAG",
        "2 / 100",
        "Weight 100 / 2,800",
        "VAULT",
        "1 / 600",
        "Deposit loot (1)",
        "Top up stacks (1)",
        "Type",
        "1,284,440 z",
        "Select an item \u{b7} Drag it across, or double-click to move the whole stack",
    ] {
        assert!(
            shown.iter().any(|text| text == expected),
            "{expected}: {shown:?}"
        );
    }
    assert_eq!(count::<With<StorageCell>>(&mut app), 3);
    assert_eq!(count::<With<StoragePane>>(&mut app), 2);
    assert!(
        !shown.iter().any(|text| text == "Undo"),
        "nothing to undo yet"
    );
}

#[test]
fn a_new_amount_rewrites_the_bar_in_place_and_typing_sets_it() {
    let mut app = window_app();
    open_window(&mut app, vec![bag(7, POTION, 0, 40)], vec![]);
    {
        let mut ui = app.world_mut().resource_mut::<StorageUi>();
        ui.selected = Some(StorageSelection::Bag(7));
        ui.qty = 40;
    }
    app.update();
    app.update();

    let field = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<StorageAmountField>>();
        query.single(world).expect("a stack gets the amount field")
    };
    let field_text = |app: &App| {
        app.world()
            .get::<EditableText>(field)
            .expect("the field outlives a new amount")
            .value()
            .to_string()
    };
    assert_eq!(field_text(&app), "40");
    assert!(texts(&mut app).iter().any(|text| text == "Deposit 40"));

    app.world_mut().resource_mut::<StorageUi>().qty = 12;
    app.update();
    assert_eq!(field_text(&app), "12");
    assert!(texts(&mut app).iter().any(|text| text == "Deposit 12"));

    set_field_text(
        &mut app.world_mut().get_mut::<EditableText>(field).unwrap(),
        "99",
    );
    app.update();
    app.update();
    assert_eq!(ui(&app).qty, 40, "a typed amount is capped to the stack");
    assert_eq!(field_text(&app), "40");
}

#[test]
fn a_selection_whose_slot_is_gone_is_dropped() {
    let mut app = window_app();
    open_window(&mut app, vec![bag(7, POTION, 0, 40)], vec![]);
    {
        let mut ui = app.world_mut().resource_mut::<StorageUi>();
        ui.selected = Some(StorageSelection::Bag(7));
        ui.asking = true;
    }
    app.update();

    *app.world_mut().resource_mut::<Inventory>() = Inventory::default();
    app.update();

    assert_eq!(ui(&app).selected, None);
    assert!(!ui(&app).asking);
}

#[test]
fn a_toast_shows_its_text_and_goes_away_when_its_time_is_up() {
    let mut app = window_app();
    open_window(&mut app, vec![], vec![]);
    let now = app.world().resource::<Time>().elapsed();
    app.world_mut().resource_mut::<StorageUi>().toast = Some(Toast {
        text: "Storage is full.".to_string(),
        bad: true,
        until: now + Duration::from_secs(3600),
    });
    app.update();

    let pill = |app: &mut App| {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&Visibility, With<StorageToast>>();
        *query.single(world).unwrap()
    };
    assert_eq!(pill(&mut app), Visibility::Inherited);
    assert!(
        texts(&mut app)
            .iter()
            .any(|text| text == "Storage is full.")
    );

    app.world_mut()
        .resource_mut::<StorageUi>()
        .toast
        .as_mut()
        .unwrap()
        .until = Duration::ZERO;
    app.update();

    assert_eq!(pill(&mut app), Visibility::Hidden);
    assert_eq!(ui(&app).toast, None);
}

#[test]
fn leaving_gameplay_drops_the_run_in_progress() {
    let mut app = App::new();
    app.init_resource::<StorageUi>();
    app.world_mut().resource_mut::<StorageUi>().run = Some(flying(StorageSelection::Bag(2), 5));

    app.world_mut().run_system_once(reset_storage_ui).unwrap();

    assert_eq!(ui(&app), &StorageUi::default());
}
