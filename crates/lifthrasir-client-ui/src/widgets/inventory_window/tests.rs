use super::*;
use bevy::scene::ScenePlugin;
use lifthrasir_data::{ItemData, ItemInfo};

fn item(index: u16, item_type: u8) -> Item {
    Item {
        index,
        item_id: 500 + u32::from(index),
        item_type,
        amount: 1,
        identified: true,
        ..Default::default()
    }
}

/// Red Potion (Use), Apple (Use), a three-slot Blade (Equip) and Jellopy (Etc).
fn mixed_inventory() -> Inventory {
    let mut inventory = Inventory::default();
    inventory.upsert(item(2, 0));
    inventory.upsert(item(3, 2));
    inventory.upsert(item(4, 5));
    inventory.upsert(item(5, 3));
    inventory
}

fn item_db() -> ItemDb {
    let mut data = ItemData::default();
    for (id, name, slot_count) in [
        (502, "Red Potion", 0),
        (503, "Apple", 0),
        (504, "Blade", 3),
        (505, "Jellopy", 0),
    ] {
        data.items.insert(
            id,
            ItemInfo {
                identified_name: name.to_string(),
                slot_count,
                ..Default::default()
            },
        );
    }
    ItemDb::from_item_data(data)
}

fn shown(inventory: &Inventory, ui: &InventoryUi) -> Vec<u16> {
    visible_items(inventory, ui, Some(&item_db()))
        .iter()
        .map(|item| item.index)
        .collect()
}

fn ui_on(tab: InventoryTab) -> InventoryUi {
    InventoryUi {
        tab,
        ..Default::default()
    }
}

#[test]
fn tabs_filter_by_category_and_favorite() {
    let mut inventory = mixed_inventory();
    inventory.upsert(Item {
        favorite: true,
        ..item(6, 3)
    });
    inventory.upsert(Item {
        wear_state: 0x0002,
        ..item(7, 5)
    });

    assert_eq!(shown(&inventory, &ui_on(InventoryTab::Use)), [2, 3]);
    assert_eq!(shown(&inventory, &ui_on(InventoryTab::Equip)), [4]);
    assert_eq!(shown(&inventory, &ui_on(InventoryTab::Etc)), [5, 6]);
    assert_eq!(shown(&inventory, &ui_on(InventoryTab::Favorites)), [6]);
    assert_eq!(
        shown(&inventory, &ui_on(InventoryTab::All)),
        [2, 3, 4, 5, 6],
        "worn items stay out of the bag"
    );
}

#[test]
fn search_matches_the_shown_name_ignoring_case() {
    let inventory = mixed_inventory();
    let search = |query: &str| InventoryUi {
        query: query.to_string(),
        ..Default::default()
    };

    assert_eq!(shown(&inventory, &search("POTION")), [2]);
    assert_eq!(shown(&inventory, &search("[3]")), [4]);
    assert!(shown(&inventory, &search("sword")).is_empty());
}

#[test]
fn sort_orders_by_type_then_index_or_by_name() {
    let mut inventory = mixed_inventory();
    inventory.upsert(item(6, 0));
    let by = |sort| InventoryUi {
        sort,
        ..Default::default()
    };

    assert_eq!(shown(&inventory, &by(InventorySort::Type)), [2, 3, 6, 4, 5]);
    assert_eq!(shown(&inventory, &by(InventorySort::Name)), [3, 4, 6, 5, 2]);
}

#[test]
fn grid_draws_every_bag_slot_unfiltered_and_whole_rows_when_filtered() {
    assert_eq!(slot_total(4, &InventoryUi::default()), CAPACITY);
    assert_eq!(slot_total(3, &ui_on(InventoryTab::Use)), MIN_SLOTS);
    assert_eq!(slot_total(41, &ui_on(InventoryTab::Use)), 48);
}

#[test]
fn is_double_click_respects_index_and_window() {
    let last = LastCellClick {
        index: 5,
        at: Duration::from_millis(100),
    };
    assert!(is_double_click(&last, 5, Duration::from_millis(350)));
    assert!(!is_double_click(&last, 6, Duration::from_millis(200)));
    assert!(!is_double_click(&last, 5, Duration::from_millis(500)));
}

/// An app holding the bag and the rebuild system, with no body mount yet.
fn bag_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app.init_resource::<InventoryUi>();
    app.insert_resource(mixed_inventory());
    app.insert_resource(item_db());
    app.add_systems(Update, rebuild_body);
    app
}

fn body_app() -> App {
    let mut app = bag_app();
    app.world_mut().spawn(InventoryBody);
    app
}

fn cell_count(app: &mut App) -> usize {
    let world = app.world_mut();
    world
        .query_filtered::<(), With<InventoryCell>>()
        .iter(world)
        .count()
}

fn texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world
        .query::<&Text>()
        .iter(world)
        .map(|text| text.0.clone())
        .collect()
}

fn grid(app: &mut App) -> Entity {
    let world = app.world_mut();
    world
        .query_filtered::<Entity, With<InventoryGrid>>()
        .single(world)
        .expect("one grid")
}

#[test]
fn window_spawns_hidden_and_fills_its_mounts() {
    let mut app = bag_app();
    app.add_systems(Update, rebuild_footer);
    app.world_mut().spawn((
        LocalPlayer,
        CharacterStatus {
            weight: 100,
            max_weight: 2000,
            ..default()
        },
    ));
    let hud = app.world_mut().spawn_empty().id();
    scene::build(&mut app.world_mut().commands(), hud);
    app.world_mut().flush();
    app.update();

    let world = app.world_mut();
    let root = world
        .query_filtered::<&Visibility, With<InventoryWindowRoot>>()
        .single(world)
        .expect("one window");
    assert_eq!(*root, Visibility::Hidden);
    let searches = world
        .query_filtered::<(), (With<InventorySearchField>, With<EditableText>)>()
        .iter(world)
        .count();
    assert_eq!(searches, 1);
    let texts = texts(&mut app);
    for expected in [
        "Inventory",
        "4 / 100",
        "Type",
        "Search items",
        "Favorites",
        "Light",
    ] {
        assert!(
            texts.iter().any(|t| t == expected),
            "{expected:?} missing from {texts:?}"
        );
    }
}

#[test]
fn rebuild_draws_one_cell_per_item_of_the_active_tab() {
    let mut app = body_app();
    app.update();
    assert_eq!(cell_count(&mut app), 4, "All shows the whole bag");

    app.world_mut().resource_mut::<InventoryUi>().tab = InventoryTab::Equip;
    app.update();
    assert_eq!(cell_count(&mut app), 1, "Equip shows its single item");
}

#[test]
fn rebuild_keeps_the_frame_labels_current() {
    let mut app = body_app();
    let used = app
        .world_mut()
        .spawn((Text::default(), InventoryLabel::Used))
        .id();
    let sort = app
        .world_mut()
        .spawn((Text::default(), InventoryLabel::Sort))
        .id();
    app.update();

    assert_eq!(app.world().get::<Text>(used).unwrap().0, "4 / 100");
    assert_eq!(app.world().get::<Text>(sort).unwrap().0, "Type");
}

#[test]
fn selection_bar_swaps_its_hint_for_the_selected_item() {
    let mut app = body_app();
    app.update();
    let hint = "Select an item \u{b7} Right-click it for details".to_string();
    assert!(texts(&mut app).contains(&hint));

    app.world_mut().resource_mut::<InventoryUi>().selected = Some(4);
    app.update();

    let texts = texts(&mut app);
    assert!(!texts.contains(&hint), "{texts:?}");
    for expected in ["Blade [3]", "Equipment", "Equip", "Drop"] {
        assert!(
            texts.iter().any(|t| t == expected),
            "{expected:?} missing from {texts:?}"
        );
    }
}

#[test]
fn empty_search_result_says_so() {
    let mut app = body_app();
    app.world_mut().resource_mut::<InventoryUi>().query = "sword".to_string();
    app.update();

    assert_eq!(cell_count(&mut app), 0);
    assert!(texts(&mut app).contains(&"No items match \u{201c}sword\u{201d}".to_string()));
}

#[test]
fn rebuild_keeps_scroll_on_inventory_change_and_resets_it_on_a_new_filter() {
    let mut app = body_app();
    app.update();
    let first = grid(&mut app);
    app.world_mut()
        .entity_mut(first)
        .insert(ScrollPosition(Vec2::new(0.0, 120.0)));

    app.world_mut()
        .resource_mut::<Inventory>()
        .upsert(item(9, 0));
    app.update();
    let second = grid(&mut app);
    assert_ne!(first, second);
    assert_eq!(app.world().get::<ScrollPosition>(second).unwrap().y, 120.0);

    app.world_mut().resource_mut::<InventoryUi>().tab = InventoryTab::Use;
    app.update();
    let third = grid(&mut app);
    assert_eq!(app.world().get::<ScrollPosition>(third).unwrap().y, 0.0);
}

#[test]
fn footer_shows_weight_tier_and_zeny() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app.add_systems(Update, rebuild_footer);
    app.world_mut().spawn(InventoryFooter);
    app.world_mut().spawn((
        LocalPlayer,
        CharacterStatus {
            weight: 1400,
            max_weight: 2800,
            zeny: 1_284_500,
            ..default()
        },
    ));
    app.update();

    let texts = texts(&mut app);
    for expected in ["1,400 / 2,800", "Heavy \u{b7} No regen", "1,284,500 z"] {
        assert!(
            texts.iter().any(|t| t == expected),
            "{expected:?} missing from {texts:?}"
        );
    }
}

#[test]
fn open_state_drives_the_root_visibility() {
    let mut app = App::new();
    app.init_resource::<InventoryUi>();
    app.add_systems(Update, reflect_open);
    let root = app
        .world_mut()
        .spawn((InventoryWindowRoot, Visibility::Hidden))
        .id();

    app.world_mut().resource_mut::<InventoryUi>().open = true;
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(root),
        Some(&Visibility::Visible)
    );

    app.world_mut().resource_mut::<InventoryUi>().open = false;
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(root),
        Some(&Visibility::Hidden)
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

/// An app holding the bag, with one slot for the item at `index`.
fn click_app(index: u16) -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_message::<UseItemRequested>();
    app.add_message::<EquipItemRequested>();
    app.add_message::<ShowInfoModal>();
    app.init_resource::<InventoryUi>();
    app.init_resource::<LastCellClick>();
    app.init_resource::<Time>();
    app.insert_resource(mixed_inventory());
    app.insert_resource(item_db());
    let cell = app
        .world_mut()
        .spawn(InventoryCell { index })
        .observe(on_cell_click)
        .id();
    let window = app.world_mut().spawn_empty().id();
    (app, cell, window)
}

fn written<M: Message + Clone>(app: &App) -> Vec<M> {
    app.world()
        .resource::<Messages<M>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}

#[test]
fn secondary_click_opens_the_info_modal_without_selecting() {
    let (mut app, cell, window) = click_app(2);

    app.world_mut()
        .trigger(click_event(cell, window, PointerButton::Secondary));

    let shown = written::<ShowInfoModal>(&app);
    assert_eq!(shown.len(), 1);
    assert!(matches!(
        &shown[0].content,
        InfoContent::Item { view, action: Some(ItemAction { index: 2, .. }) }
            if view.item_id == 502
    ));
    assert_eq!(app.world().resource::<InventoryUi>().selected, None);
}

#[test]
fn click_on_a_slot_whose_item_is_gone_does_nothing() {
    let (mut app, cell, window) = click_app(40);

    app.world_mut()
        .trigger(click_event(cell, window, PointerButton::Secondary));
    app.world_mut()
        .trigger(click_event(cell, window, PointerButton::Primary));

    assert!(written::<ShowInfoModal>(&app).is_empty());
    assert_eq!(app.world().resource::<InventoryUi>().selected, None);
}

#[test]
fn primary_click_selects_without_acting_or_opening_the_modal() {
    let (mut app, cell, window) = click_app(2);
    // A first click never counts as a double-click of the default `LastCellClick`.
    app.world_mut().resource_mut::<LastCellClick>().index = u16::MAX;

    app.world_mut()
        .trigger(click_event(cell, window, PointerButton::Primary));

    assert_eq!(app.world().resource::<InventoryUi>().selected, Some(2));
    assert!(written::<ShowInfoModal>(&app).is_empty());
    assert!(written::<UseItemRequested>(&app).is_empty());
}

#[test]
fn double_click_uses_a_usable_item() {
    let (mut app, cell, window) = click_app(2);
    app.world_mut().resource_mut::<LastCellClick>().index = u16::MAX;

    for _ in 0..2 {
        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Primary));
    }

    let used = written::<UseItemRequested>(&app);
    assert_eq!(used.len(), 1);
    assert_eq!(used[0].index, 2);
}

#[test]
fn double_click_equips_gear_and_ignores_plain_etc_items() {
    let (mut app, blade, window) = click_app(4);
    let jellopy = app
        .world_mut()
        .spawn(InventoryCell { index: 5 })
        .observe(on_cell_click)
        .id();
    app.world_mut().resource_mut::<LastCellClick>().index = u16::MAX;

    for cell in [blade, blade, jellopy, jellopy] {
        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Primary));
    }

    let equipped = written::<EquipItemRequested>(&app);
    assert_eq!(equipped.len(), 1);
    assert_eq!(equipped[0].index, 4);
    assert!(written::<UseItemRequested>(&app).is_empty());
}

#[test]
fn action_button_acts_on_the_selected_item() {
    let (mut app, _, _) = click_app(2);
    app.world_mut().resource_mut::<InventoryUi>().selected = Some(3);
    let button = app.world_mut().spawn_empty().observe(act_on_selection).id();

    app.world_mut().trigger(Activate { entity: button });

    let used = written::<UseItemRequested>(&app);
    assert_eq!(used.len(), 1);
    assert_eq!(used[0].index, 3);
}

#[test]
fn slot_name_tag_is_drawn_outside_the_grid_clip() {
    let mut app = bag_app();
    let slot = app.world_mut().spawn_empty().id();
    app.world_mut()
        .spawn_scene(scene::slot_tooltip("Blade [3]".to_string()))
        .expect("scene spawns")
        .insert(ChildOf(slot));
    app.update();

    let world = app.world_mut();
    let tags = world
        .query_filtered::<&Pickable, (With<InventoryTooltip>, With<bevy::ui::OverrideClip>)>()
        .single(world)
        .expect("one name tag");
    assert!(
        !tags.is_hoverable,
        "the tag must not steal the slot's hover"
    );
    assert!(texts(&mut app).contains(&"Blade [3]".to_string()));
}

#[test]
fn tab_click_switches_tab_and_close_button_closes() {
    let mut app = App::new();
    app.insert_resource(InventoryUi {
        open: true,
        ..Default::default()
    });
    let tab = app
        .world_mut()
        .spawn(InventoryTabButton(InventoryTab::Etc))
        .observe(pick_tab)
        .id();
    let close = app.world_mut().spawn_empty().observe(close_window).id();
    let window = app.world_mut().spawn_empty().id();

    app.world_mut()
        .trigger(click_event(tab, window, PointerButton::Primary));
    assert_eq!(app.world().resource::<InventoryUi>().tab, InventoryTab::Etc);

    app.world_mut().trigger(Activate { entity: close });
    assert!(!app.world().resource::<InventoryUi>().open);
}
