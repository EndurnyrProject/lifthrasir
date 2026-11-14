use std::time::Duration;

use bevy::asset::AssetPlugin;
use bevy::scene::ScenePlugin;
use lifthrasir_client::domain::entities::character::components::core::CharacterStats;
use lifthrasir_client::domain::entities::character::states::status_icons::ActiveStatus;
use lifthrasir_client::domain::hotbar::SLOTS;
use lifthrasir_client::domain::inventory::Item;
use lifthrasir_client::domain::skill::SkillNode;
use net_contract::events::SkillCooldownSet;

use super::view::{Choice, Tone, choices, cooled_down, edge, page_label, refusal, slot_view, tip};
use super::*;

/// Enemy-targeted, so the skill is cast rather than passive.
const CAST: u32 = 1;
const PASSIVE: u32 = 0;
/// A usable item's type, and one that is not usable.
const USABLE: u8 = 0;
const ETC: u8 = 3;

fn node(level: u32, sp: u32, inf_type: u32) -> SkillNode {
    SkillNode {
        level,
        max_level: 10,
        upgradable: true,
        requires: vec![],
        req_base_level: 0,
        req_job_level: 0,
        sp,
        range: 0,
        inf_type,
        job_id: 7,
        splash_radius: 0,
        cast_time: vec![],
        cooldown: vec![],
    }
}

fn tree(entries: Vec<(u32, SkillNode)>) -> SkillTreeState {
    SkillTreeState {
        skills: entries.into_iter().collect(),
    }
}

fn bag(items: &[(u16, u32, u16, u8)]) -> Inventory {
    let mut inventory = Inventory::default();
    for &(index, item_id, amount, item_type) in items {
        inventory.upsert(Item {
            index,
            item_id,
            amount,
            item_type,
            identified: true,
            ..Default::default()
        });
    }
    inventory
}

/// The hero's skills and bag, for the pure view functions.
#[derive(Default)]
struct Hero {
    inventory: Inventory,
    tree: SkillTreeState,
    cast_levels: SkillCastLevels,
}

impl Hero {
    fn facts(&self, sp: u32) -> Facts<'_> {
        Facts {
            inventory: &self.inventory,
            catalog: None,
            items: None,
            tree: &self.tree,
            cast_levels: &self.cast_levels,
            sp,
        }
    }

    fn slot(&self, slot: HotbarSlot, sp: u32, live: Live) -> Filled {
        view::filled(slot, &self.facts(sp), live)
    }
}

fn bash_hero() -> Hero {
    Hero {
        tree: tree(vec![(5, node(7, 15, CAST))]),
        inventory: bag(&[(2, 501, 40, USABLE)]),
        ..Default::default()
    }
}

#[test]
fn an_empty_slot_only_names_its_key() {
    let view = slot_view(
        None,
        "F1".into(),
        &Hero::default().facts(0),
        Live::default(),
    );
    assert_eq!(view.key, "F1");
    assert_eq!(view.filled, None);
    assert_eq!(tip(&view, false), None);
}

#[test]
fn a_skill_shows_its_cast_level_and_what_it_costs() {
    let mut hero = bash_hero();
    let filled = hero.slot(HotbarSlot::Skill(5), 100, Live::default());
    assert_eq!((filled.level, filled.sp), (Some(7), Some(15)));
    assert!(!filled.lacks_sp && !filled.spent());

    hero.cast_levels.set(5, 3, 7);
    assert_eq!(
        hero.slot(HotbarSlot::Skill(5), 100, Live::default()).level,
        Some(3),
        "the slot casts at the level picked in the Skills window"
    );

    let broke = hero.slot(HotbarSlot::Skill(5), 14, Live::default());
    assert!(broke.lacks_sp && broke.spent());
}

#[test]
fn an_unlearned_skill_and_an_item_no_longer_held_are_spent() {
    let hero = bash_hero();
    let unlearned = hero.slot(HotbarSlot::Skill(99), 100, Live::default());
    assert_eq!((unlearned.level, unlearned.sp), (None, None));
    assert!(unlearned.spent());

    let held = hero.slot(HotbarSlot::Item(501), 0, Live::default());
    assert_eq!(held.count, Some(40));
    assert!(!held.spent());

    let gone = hero.slot(HotbarSlot::Item(777), 0, Live::default());
    assert_eq!(gone.count, Some(0));
    assert!(gone.spent());
}

#[test]
fn a_cooldown_counts_whole_seconds_then_tenths() {
    let hero = bash_hero();
    let left = |secs| {
        let live = Live {
            cooldown: Some(secs),
            ..Default::default()
        };
        hero.slot(HotbarSlot::Skill(5), 100, live).cooldown
    };
    assert_eq!(left(2.3).as_deref(), Some("3"));
    assert_eq!(left(1.0).as_deref(), Some("1"));
    assert_eq!(left(0.44).as_deref(), Some("0.4"));
}

#[test]
fn a_skill_coming_off_cooldown_is_noticed_once() {
    let hero = bash_hero();
    let view = |cooldown| {
        let live = Live {
            cooldown,
            ..Default::default()
        };
        slot_view(
            Some(HotbarSlot::Skill(5)),
            String::new(),
            &hero.facts(100),
            live,
        )
    };
    let (cooling, ready) = (view(Some(0.2)), view(None));
    assert!(cooled_down(Some(&cooling), &ready));
    assert!(!cooled_down(Some(&ready), &ready));
    assert!(!cooled_down(None, &ready));
    let other = slot_view(
        Some(HotbarSlot::Item(501)),
        String::new(),
        &hero.facts(100),
        Live::default(),
    );
    assert!(
        !cooled_down(Some(&cooling), &other),
        "another slot's content"
    );
}

#[test]
fn a_refused_use_says_the_first_thing_in_the_way() {
    let hero = bash_hero();
    let skill = |sp, cooldown| {
        let live = Live {
            cooldown,
            ..Default::default()
        };
        hero.slot(HotbarSlot::Skill(5), sp, live)
    };
    assert_eq!(refusal(&skill(100, None), None), None);
    assert_eq!(
        refusal(&skill(1, Some(1.4)), Some("Bolt")).as_deref(),
        Some("#5 is ready in 2s")
    );
    assert_eq!(
        refusal(&skill(1, None), Some("Bolt")).as_deref(),
        Some("Already casting Bolt")
    );
    assert_eq!(
        refusal(&skill(1, None), None).as_deref(),
        Some("Not enough SP \u{b7} #5 needs 15")
    );
    let unlearned = hero.slot(HotbarSlot::Skill(99), 100, Live::default());
    assert_eq!(
        refusal(&unlearned, None).as_deref(),
        Some("#99 is not learned")
    );

    let gone = hero.slot(HotbarSlot::Item(777), 0, Live::default());
    assert_eq!(
        refusal(&gone, Some("Bolt")).as_deref(),
        Some("You have no Item #777 left")
    );
    let held = hero.slot(HotbarSlot::Item(501), 0, Live::default());
    assert_eq!(
        refusal(&held, Some("Bolt")),
        None,
        "a cast does not stop an item"
    );
}

#[test]
fn the_outline_follows_drops_statuses_and_the_pointer() {
    let hero = bash_hero();
    let empty = SlotView::default();
    let skill = |buff| {
        let live = Live {
            buff,
            ..Default::default()
        };
        slot_view(
            Some(HotbarSlot::Skill(5)),
            String::new(),
            &hero.facts(100),
            live,
        )
    };
    let (plain, active) = (skill(None), skill(Some(Duration::from_secs(90))));
    assert_eq!(edge(&empty, false, false), Edge::Empty);
    assert_eq!(edge(&empty, true, false), Edge::EmptyHover);
    assert_eq!(edge(&plain, false, false), Edge::Rest);
    assert_eq!(edge(&plain, true, false), Edge::Hover);
    assert_eq!(edge(&active, true, false), Edge::Active);
    assert_eq!(edge(&active, true, true), Edge::DropTarget);
    assert_eq!(active.filled.unwrap().buff.as_deref(), Some("1:30"));
}

#[test]
fn the_tooltip_lists_what_the_client_knows_and_how_to_use_the_slot() {
    let hero = bash_hero();
    let skill = slot_view(
        Some(HotbarSlot::Skill(5)),
        "F2".into(),
        &hero.facts(1),
        Live::default(),
    );
    let view = tip(&skill, false).unwrap();
    assert_eq!((view.name.as_str(), view.tag), ("#5", "SKILL"));
    assert_eq!(
        view.stats,
        vec![
            ("Level", "7".to_string(), ro_dark::STRONG),
            ("SP", "15".to_string(), view::SHORT_INK),
            ("Cast", "Instant".to_string(), ro_dark::STRONG),
        ]
    );
    assert_eq!(
        view.hint,
        "Press F2 or click \u{b7} Right-click to remove \u{b7} Drag to move"
    );

    let item = slot_view(
        Some(HotbarSlot::Item(501)),
        String::new(),
        &hero.facts(1),
        Live::default(),
    );
    let view = tip(&item, true).unwrap();
    assert_eq!(view.tag, "ITEM");
    assert_eq!(
        view.stats,
        vec![("In bag", "40".to_string(), ro_dark::STRONG)]
    );
    assert_eq!(view.hint, "Click", "a locked bar is only used");
}

#[test]
fn the_tooltip_times_a_skill_at_the_level_it_is_cast_at() {
    let mut bolt = node(3, 16, CAST);
    bolt.cast_time = vec![800, 1200, 1600];
    bolt.cooldown = vec![0, 0, 2500];
    let mut hero = Hero {
        tree: tree(vec![(19, bolt)]),
        ..Default::default()
    };
    let timing = |hero: &Hero| {
        let slot = slot_view(
            Some(HotbarSlot::Skill(19)),
            String::new(),
            &hero.facts(100),
            Live::default(),
        );
        let stats = tip(&slot, false).unwrap().stats;
        stats
            .into_iter()
            .filter(|(label, ..)| ["Cast", "Cooldown"].contains(label))
            .map(|(label, value, _)| (label, value))
            .collect::<Vec<_>>()
    };

    assert_eq!(
        timing(&hero),
        vec![
            ("Cast", "1.6s".to_string()),
            ("Cooldown", "2.5s".to_string())
        ]
    );

    hero.cast_levels.set(19, 2, 3);
    assert_eq!(timing(&hero), vec![("Cast", "1.2s".to_string())]);

    let potion = slot_view(
        Some(HotbarSlot::Item(501)),
        String::new(),
        &hero.facts(100),
        Live::default(),
    );
    assert_eq!(potion.filled.unwrap().timing, None, "items have no cast");
}

#[test]
fn the_picker_offers_cast_skills_and_usable_items_once_each() {
    let hero = Hero {
        tree: tree(vec![
            (5, node(7, 15, CAST)),
            (6, node(0, 0, CAST)),
            (7, node(3, 0, PASSIVE)),
            (3, node(1, 8, CAST)),
        ]),
        inventory: bag(&[
            (2, 501, 40, USABLE),
            (3, 909, 5, ETC),
            (4, 502, 1, USABLE),
            (5, 501, 2, USABLE),
        ]),
        ..Default::default()
    };
    let mut hotbar = Hotbar::default();
    hotbar.assign(SLOTS + 1, HotbarSlot::Skill(5));

    let row = |choice: &Choice| (choice.slot, choice.meta.clone(), choice.on_bar);
    let skills = choices(PickTab::Skills, &hotbar, &hero.facts(0));
    assert_eq!(
        skills.iter().map(row).collect::<Vec<_>>(),
        vec![
            (HotbarSlot::Skill(3), "Lv 1".to_string(), false),
            (HotbarSlot::Skill(5), "On bar".to_string(), true),
        ],
        "unlearned and passive skills are left out; another page still counts"
    );

    let items = choices(PickTab::Items, &hotbar, &hero.facts(0));
    assert_eq!(
        items.iter().map(row).collect::<Vec<_>>(),
        vec![
            (HotbarSlot::Item(501), "\u{d7}40".to_string(), false),
            (HotbarSlot::Item(502), "\u{d7}1".to_string(), false),
        ]
    );
}

#[test]
fn the_pager_names_the_pages_on_show() {
    let mut hotbar = Hotbar::default();
    assert_eq!(page_label(&hotbar), "1 / 3");
    hotbar.two_rows = true;
    assert_eq!(page_label(&hotbar), "1\u{b7}2");
    hotbar.turn(false);
    assert_eq!(page_label(&hotbar), "3\u{b7}1");
}

#[test]
fn toasts_word_and_tone_each_notice() {
    let potion = HotbarSlot::Item(501);
    let cases = [
        (Notice::Refused("No".into()), "No", Tone::Warn),
        (Notice::Locked, "Unlock the hotbar to change it", Tone::Warn),
        (Notice::LockSet(false), "Hotbar unlocked", Tone::Info),
        (
            Notice::Removed(potion, Some((0, 2))),
            "Removed Potion from F3",
            Tone::Info,
        ),
        (
            Notice::Placed(potion, (0, 2)),
            "Potion added to F3",
            Tone::Good,
        ),
    ];
    for (notice, text, tone) in cases {
        assert_eq!(notice.text("Potion", "F3"), text);
        assert_eq!(notice.tone(), tone);
    }
    assert_eq!(
        Notice::Removed(potion, None).text("Potion", ""),
        "Removed Potion"
    );
    assert_eq!(Notice::Locked.subject(), None);
    assert_eq!(
        Notice::Placed(potion, (1, 4)).subject(),
        Some((potion, Some((1, 4))))
    );
}

#[test]
fn popups_hang_over_their_slot_and_above_the_rows_under_it() {
    let first = scene::anchor(0, 0, 1);
    assert_eq!(first, Vec2::new(104.0, 60.0));
    assert_eq!(
        scene::anchor(0, 4, 1).x - scene::anchor(0, 3, 1).x,
        54.0,
        "a group gap"
    );
    assert_eq!(
        scene::anchor(0, 0, 2).y - scene::anchor(1, 0, 2).y,
        50.0,
        "the upper row stands one row higher"
    );
    assert_eq!(scene::anchor(1, 0, 2).y, first.y);
}

#[test]
fn drop_source_classifies_swap_and_place() {
    assert_eq!(drop_source(Some(3), None), Some(DropSource::Swap(3)));
    assert_eq!(
        drop_source(None, Some(HotbarSlot::Skill(7))),
        Some(DropSource::Place(HotbarSlot::Skill(7)))
    );
    assert_eq!(drop_source(None, None), None);
    assert_eq!(
        drop_source(Some(3), Some(HotbarSlot::Item(1))),
        Some(DropSource::Swap(3))
    );
}

#[test]
fn a_drop_places_overwrites_or_swaps_across_pages() {
    let mut bar = Hotbar::default();
    apply_drop(&mut bar, 2, DropSource::Place(HotbarSlot::Skill(1)));
    apply_drop(&mut bar, 2, DropSource::Place(HotbarSlot::Item(501)));
    assert_eq!(bar.get(2), Some(HotbarSlot::Item(501)));

    bar.assign(0, HotbarSlot::Skill(1));
    apply_drop(&mut bar, SLOTS + 1, DropSource::Swap(0));
    assert_eq!(bar.get(0), None);
    assert_eq!(bar.get(SLOTS + 1), Some(HotbarSlot::Skill(1)));

    apply_drop(&mut bar, 2, DropSource::Swap(SLOTS + 1));
    assert_eq!(bar.get(2), Some(HotbarSlot::Skill(1)));
    assert_eq!(bar.get(SLOTS + 1), Some(HotbarSlot::Item(501)));

    let before = bar.clone();
    apply_drop(&mut bar, 999, DropSource::Place(HotbarSlot::Skill(1)));
    apply_drop(&mut bar, 999, DropSource::Swap(0));
    assert_eq!(bar, before, "out of range is safe");
}

#[test]
fn only_a_bar_drag_that_missed_every_slot_unregisters() {
    assert_eq!(unregister_target(Some(4), false), Some(4));
    assert_eq!(unregister_target(Some(4), true), None);
    assert_eq!(unregister_target(None, false), None);
    assert_eq!(unregister_target(None, true), None);
}

fn pointer(window: Entity) -> Pointer {
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

fn click(app: &mut App, target: Entity, button: PointerButton) {
    use bevy::picking::backend::HitData;
    let window = app.world_mut().spawn_empty().id();
    app.world_mut().trigger(PointerClick {
        entity: target,
        pointer: pointer(window),
        button,
        hit: HitData::new(target, 0.0, None, None),
        duration: Duration::ZERO,
        count: 1,
    });
}

fn drag_start(app: &mut App, target: Entity) {
    use bevy::picking::backend::HitData;
    let window = app.world_mut().spawn_empty().id();
    app.world_mut().trigger(PointerDragStart {
        entity: target,
        pointer: pointer(window),
        button: PointerButton::Primary,
        hit: HitData::new(target, 0.0, None, None),
    });
}

fn drag_end(app: &mut App) {
    let target = app.world_mut().spawn_empty().id();
    let window = app.world_mut().spawn_empty().id();
    app.world_mut().trigger(PointerDragEnd {
        entity: target,
        pointer: pointer(window),
        button: PointerButton::Primary,
        distance: Vec2::ZERO,
    });
}

/// The bar's state and a cell wired to its pointer observers, with nothing drawn.
fn wired_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_message::<HotbarSlotActivated>();
    app.init_resource::<Hotbar>()
        .init_resource::<HotbarUi>()
        .init_resource::<HotbarDrag>()
        .add_observer(reset_drag);
    app
}

fn wired_cell(app: &mut App, row: usize, col: usize) -> Entity {
    app.world_mut()
        .spawn(HotbarSlotUi { row, col })
        .observe(on_slot_click)
        .observe(on_slot_drag_start)
        .id()
}

fn press(app: &mut App, action: HotbarAction) {
    let button = app.world_mut().spawn(action).observe(on_action).id();
    app.world_mut().trigger(Activate { entity: button });
}

fn activations(app: &App) -> Vec<usize> {
    app.world()
        .resource::<Messages<HotbarSlotActivated>>()
        .iter_current_update_messages()
        .map(|message| message.index)
        .collect()
}

fn hotbar(app: &mut App) -> Mut<'_, Hotbar> {
    app.world_mut().resource_mut::<Hotbar>()
}

fn notice(app: &App) -> Option<Notice> {
    let toast = app.world().resource::<HotbarUi>().toast.as_ref()?;
    Some(toast.notice.clone())
}

#[test]
fn a_click_uses_the_slot_of_the_page_on_show() {
    let mut app = wired_app();
    hotbar(&mut app).assign(SLOTS + 4, HotbarSlot::Skill(10));
    hotbar(&mut app).page = 1;
    let cell = wired_cell(&mut app, 0, 4);

    click(&mut app, cell, PointerButton::Primary);

    assert_eq!(activations(&app), vec![SLOTS + 4]);
    assert_eq!(app.world().resource::<HotbarUi>().picker, None);
}

#[test]
fn a_click_on_an_empty_slot_opens_the_picker_unless_the_bar_is_locked() {
    let mut app = wired_app();
    hotbar(&mut app).two_rows = true;
    let cell = wired_cell(&mut app, 1, 4);

    click(&mut app, cell, PointerButton::Primary);
    assert!(activations(&app).is_empty());
    assert_eq!(
        app.world().resource::<HotbarUi>().picker,
        Some(Picker {
            row: 1,
            col: 4,
            index: SLOTS + 4,
            tab: PickTab::Skills,
        })
    );

    app.world_mut().resource_mut::<HotbarUi>().picker = None;
    hotbar(&mut app).locked = true;
    click(&mut app, cell, PointerButton::Primary);
    assert_eq!(app.world().resource::<HotbarUi>().picker, None);
    assert_eq!(notice(&app), Some(Notice::Locked));
}

#[test]
fn a_right_click_empties_the_slot_and_undo_refills_it() {
    let mut app = wired_app();
    let skill = HotbarSlot::Skill(10);
    hotbar(&mut app).assign(4, skill);
    let cell = wired_cell(&mut app, 0, 4);

    click(&mut app, cell, PointerButton::Secondary);
    assert_eq!(hotbar(&mut app).get(4), None);
    assert!(activations(&app).is_empty());
    assert_eq!(notice(&app), Some(Notice::Removed(skill, Some((0, 4)))));

    press(&mut app, HotbarAction::Undo);
    assert_eq!(hotbar(&mut app).get(4), Some(skill));
    assert_eq!(notice(&app), None);
}

#[test]
fn a_locked_bar_keeps_its_slots() {
    let mut app = wired_app();
    let skill = HotbarSlot::Skill(10);
    hotbar(&mut app).assign(4, skill);
    hotbar(&mut app).locked = true;
    let cell = wired_cell(&mut app, 0, 4);

    click(&mut app, cell, PointerButton::Secondary);
    assert_eq!(hotbar(&mut app).get(4), Some(skill));
    assert_eq!(notice(&app), Some(Notice::Locked));

    drag_start(&mut app, cell);
    assert_eq!(app.world().resource::<HotbarDrag>().payload, None);

    click(&mut app, cell, PointerButton::Primary);
    assert_eq!(activations(&app), vec![4], "it is still used");
}

#[test]
fn a_slot_dragged_off_the_bar_is_removed_and_can_be_put_back() {
    let mut app = wired_app();
    let skill = HotbarSlot::Skill(10);
    hotbar(&mut app).assign(SLOTS + 2, skill);
    hotbar(&mut app).page = 1;
    let cell = wired_cell(&mut app, 0, 2);

    drag_start(&mut app, cell);
    let drag = app.world().resource::<HotbarDrag>();
    assert_eq!((drag.payload, drag.source), (Some(skill), Some(SLOTS + 2)));

    drag_end(&mut app);
    assert_eq!(hotbar(&mut app).get(SLOTS + 2), None);
    assert_eq!(app.world().resource::<HotbarDrag>().payload, None);
    assert_eq!(notice(&app), Some(Notice::Removed(skill, None)));

    press(&mut app, HotbarAction::Undo);
    assert_eq!(hotbar(&mut app).get(SLOTS + 2), Some(skill));
}

#[test]
fn a_drag_dropped_on_a_slot_keeps_its_source() {
    let mut app = wired_app();
    let skill = HotbarSlot::Skill(10);
    hotbar(&mut app).assign(2, skill);
    app.insert_resource(HotbarDrag {
        payload: Some(skill),
        source: Some(2),
        dropped_on_slot: true,
    });
    app.world_mut().resource_mut::<HotbarUi>().drop_target = Some(Entity::PLACEHOLDER);

    drag_end(&mut app);

    assert_eq!(hotbar(&mut app).get(2), Some(skill));
    assert_eq!(notice(&app), None);
    assert_eq!(app.world().resource::<HotbarUi>().drop_target, None);
}

#[test]
fn the_buttons_turn_pages_switch_rows_and_lock_the_bar() {
    let mut app = wired_app();
    app.world_mut().resource_mut::<HotbarUi>().picker = Some(Picker {
        row: 0,
        col: 0,
        index: 0,
        tab: PickTab::Skills,
    });

    press(&mut app, HotbarAction::PagePrev);
    assert_eq!(hotbar(&mut app).page, 2);
    assert_eq!(
        app.world().resource::<HotbarUi>().picker,
        None,
        "the picker's slot left"
    );
    press(&mut app, HotbarAction::PageNext);
    assert_eq!(hotbar(&mut app).page, 0);

    press(&mut app, HotbarAction::ToggleRows);
    assert!(hotbar(&mut app).two_rows);

    press(&mut app, HotbarAction::ToggleLock);
    assert!(hotbar(&mut app).locked);
    assert_eq!(notice(&app), Some(Notice::LockSet(true)));
    press(&mut app, HotbarAction::ToggleLock);
    assert_eq!(notice(&app), Some(Notice::LockSet(false)));
}

#[test]
fn the_picker_switches_lists_and_fills_its_slot() {
    let mut app = wired_app();
    app.world_mut().resource_mut::<HotbarUi>().picker = Some(Picker {
        row: 0,
        col: 3,
        index: 2 * SLOTS + 3,
        tab: PickTab::Skills,
    });

    press(&mut app, HotbarAction::Tab(PickTab::Items));
    let tab = app.world().resource::<HotbarUi>().picker.map(|p| p.tab);
    assert_eq!(tab, Some(PickTab::Items));

    let potion = HotbarSlot::Item(501);
    press(&mut app, HotbarAction::Pick(potion));
    assert_eq!(hotbar(&mut app).get(2 * SLOTS + 3), Some(potion));
    assert_eq!(app.world().resource::<HotbarUi>().picker, None);
    assert_eq!(notice(&app), Some(Notice::Placed(potion, (0, 3))));
}

/// The whole bar, drawn from its scenes by its systems.
fn drawn_app() -> App {
    let mut app = wired_app();
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>().init_asset::<Font>();
    app.add_message::<SkillCastStarted>()
        .add_message::<CastCancelled>()
        .add_message::<SkillCooldownSet>();
    app.init_resource::<Inventory>()
        .init_resource::<SkillTreeState>()
        .init_resource::<SkillCastLevels>()
        .init_resource::<SkillCooldownTracker>()
        .init_resource::<LocalStatuses>()
        .init_resource::<EntityRegistry>()
        .insert_resource(Keybinds::default());
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn_scene(scene::root());
    });
    app.add_systems(
        Update,
        (
            lifthrasir_client::domain::skill::cooldown::apply_skill_cooldown,
            (track_cast, age_toast, answer_use, sync_slots, play_fx).chain(),
            animate,
            sync_panel,
        )
            .chain(),
    );
    let hero = app
        .world_mut()
        .spawn((
            CharacterStatus {
                sp: 50,
                max_sp: 100,
                ..default()
            },
            CharacterData {
                name: "Hero".to_string(),
                job_id: 7,
                level: 1,
                experience: 0,
                stats: CharacterStats::default(),
                slot: 0,
            },
            LocalPlayer,
        ))
        .id();
    app.world_mut()
        .resource_mut::<EntityRegistry>()
        .set_local_player(hero, 2_000_001);
    app
}

fn cell_at(app: &mut App, row: usize, col: usize) -> Entity {
    let world = app.world_mut();
    world
        .query::<(Entity, &HotbarSlotUi)>()
        .iter(world)
        .find(|(_, cell)| (cell.row, cell.col) == (row, col))
        .map(|(entity, _)| entity)
        .expect("the cell is drawn")
}

fn body_of(app: &mut App, cell: Entity) -> Entity {
    let world = app.world_mut();
    let bodies: Vec<Entity> = world
        .query_filtered::<(Entity, &ChildOf), With<SlotBody>>()
        .iter(world)
        .filter(|(_, parent)| parent.parent() == cell)
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(bodies.len(), 1, "a cell draws one body");
    bodies[0]
}

fn shown_view(app: &App, cell: Entity) -> SlotView {
    app.world()
        .get::<Drawn>(cell)
        .and_then(|drawn| drawn.view.clone())
        .expect("the cell was drawn")
}

fn texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world
        .query::<&Text>()
        .iter(world)
        .map(|text| text.0.clone())
        .collect()
}

fn region_is_filled(app: &mut App, region: Region) -> bool {
    let world = app.world_mut();
    world
        .query::<(&Region, Option<&Children>)>()
        .iter(world)
        .any(|(r, children)| *r == region && children.is_some_and(|c| !c.is_empty()))
}

#[test]
fn the_plate_draws_two_rows_of_cells_and_shows_the_second_on_demand() {
    let mut app = drawn_app();
    app.update();

    let world = app.world_mut();
    assert_eq!(
        world.query::<&HotbarSlotUi>().iter(world).count(),
        2 * SLOTS
    );
    let second = |app: &mut App| {
        let world = app.world_mut();
        world
            .query_filtered::<&Node, With<SecondRow>>()
            .single(world)
            .unwrap()
            .display
    };
    assert_eq!(second(&mut app), Display::None);
    let drawn = texts(&mut app);
    assert!(drawn.contains(&"1 / 3".to_string()));
    assert!(
        drawn.contains(&"SP".to_string()) && drawn.contains(&"50".to_string()),
        "the hero's SP is drawn"
    );
    assert!(!drawn.contains(&"AP".to_string()), "AP is for fourth jobs");

    press(&mut app, HotbarAction::ToggleRows);
    app.update();
    assert_eq!(second(&mut app), Display::Flex);
    assert!(texts(&mut app).contains(&"1\u{b7}2".to_string()));
    let (first, shifted) = (cell_at(&mut app, 0, 2), cell_at(&mut app, 1, 2));
    assert_eq!(shown_view(&app, first).key, "F3");
    assert_eq!(
        shown_view(&app, shifted).key,
        "S+F3",
        "Shift fires the second row"
    );
}

#[test]
fn a_cell_outlives_its_redraws_and_redraws_only_on_change() {
    let mut app = drawn_app();
    app.update();
    let cell = cell_at(&mut app, 0, 0);
    let empty = body_of(&mut app, cell);

    app.update();
    assert_eq!(body_of(&mut app, cell), empty, "nothing changed");

    hotbar(&mut app).assign(0, HotbarSlot::Item(501));
    app.update();
    assert_eq!(cell_at(&mut app, 0, 0), cell, "the cell is never respawned");
    assert_ne!(body_of(&mut app, cell), empty);
    assert_eq!(
        shown_view(&app, cell).filled.map(|filled| filled.count),
        Some(Some(0))
    );

    hotbar(&mut app).turn(true);
    app.update();
    assert_eq!(
        shown_view(&app, cell).filled,
        None,
        "the next page is empty"
    );
}

#[test]
fn hovering_a_filled_slot_hangs_its_tooltip() {
    let mut app = drawn_app();
    hotbar(&mut app).assign(0, HotbarSlot::Item(501));
    app.update();
    let cell = cell_at(&mut app, 0, 0);
    assert!(!region_is_filled(&mut app, Region::Tip));

    app.world_mut().resource_mut::<HotbarUi>().hover = Some(cell);
    app.update();
    assert!(region_is_filled(&mut app, Region::Tip));
    assert!(texts(&mut app).contains(&"Item #501".to_string()));
    assert_eq!(
        app.world().get::<Drawn>(cell).unwrap().edge,
        Some(Edge::Hover)
    );

    app.world_mut().resource_mut::<HotbarDrag>().payload = Some(HotbarSlot::Skill(1));
    app.update();
    assert!(
        !region_is_filled(&mut app, Region::Tip),
        "a drag hides the tooltip"
    );
}

#[test]
fn the_drawn_picker_fills_the_slot_it_was_opened_on() {
    let mut app = drawn_app();
    app.insert_resource(tree(vec![(5, node(7, 15, CAST))]));
    app.update();
    let cell = cell_at(&mut app, 0, 3);
    let backdrop = |app: &mut App| {
        let world = app.world_mut();
        world
            .query_filtered::<&Node, With<Backdrop>>()
            .single(world)
            .unwrap()
            .display
    };
    assert_eq!(backdrop(&mut app), Display::None);

    click(&mut app, cell, PointerButton::Primary);
    app.update();
    assert_eq!(backdrop(&mut app), Display::Flex);
    assert!(region_is_filled(&mut app, Region::Picker));

    let world = app.world_mut();
    let pick = world
        .query::<(Entity, &HotbarAction)>()
        .iter(world)
        .find(|(_, action)| **action == HotbarAction::Pick(HotbarSlot::Skill(5)))
        .map(|(entity, _)| entity)
        .expect("the learned skill is offered");
    app.world_mut().trigger(Activate { entity: pick });
    app.update();

    assert_eq!(hotbar(&mut app).get(3), Some(HotbarSlot::Skill(5)));
    assert!(!region_is_filled(&mut app, Region::Picker));
    assert_eq!(backdrop(&mut app), Display::None);
    assert!(texts(&mut app).contains(&"#5 added to F4".to_string()));
}

#[test]
fn the_drawn_toast_names_the_removal_and_its_undo_puts_the_slot_back() {
    let mut app = drawn_app();
    let potion = HotbarSlot::Item(501);
    hotbar(&mut app).assign(0, potion);
    app.update();
    let cell = cell_at(&mut app, 0, 0);

    click(&mut app, cell, PointerButton::Secondary);
    app.update();
    let drawn = texts(&mut app);
    assert!(drawn.contains(&"Removed Item #501 from F1".to_string()));
    assert!(drawn.contains(&"Undo".to_string()));

    let world = app.world_mut();
    let undo = world
        .query::<(Entity, &HotbarAction)>()
        .iter(world)
        .find(|(_, action)| **action == HotbarAction::Undo)
        .map(|(entity, _)| entity)
        .expect("the toast offers Undo");
    app.world_mut().trigger(Activate { entity: undo });
    app.update();

    assert_eq!(hotbar(&mut app).get(0), Some(potion));
    assert!(!region_is_filled(&mut app, Region::Toast));
}

#[test]
fn a_toast_goes_away_on_its_own() {
    let mut app = drawn_app();
    app.world_mut().resource_mut::<HotbarUi>().toast = Some(Toast {
        notice: Notice::Locked,
        undo: None,
        left: 0.0,
    });
    app.update();
    assert_eq!(notice(&app), None);
    assert!(!region_is_filled(&mut app, Region::Toast));
}

#[test]
fn using_a_slot_presses_it_or_says_why_it_cannot_be_used() {
    let mut app = drawn_app();
    app.insert_resource(tree(vec![(5, node(7, 15, CAST))]));
    hotbar(&mut app).assign(1, HotbarSlot::Skill(5));
    app.update();
    let cell = cell_at(&mut app, 0, 1);
    let use_slot = |app: &mut App| {
        app.world_mut()
            .write_message(HotbarSlotActivated { index: 1 });
        app.update();
        app.world().get::<Fx>(cell).map(|fx| fx.kind)
    };

    assert_eq!(use_slot(&mut app), Some(FxKind::Press));
    assert_eq!(notice(&app), None);

    app.world_mut().write_message(SkillCooldownSet {
        skill_id: 5,
        tick: 2300,
    });
    app.update();
    assert_eq!(
        shown_view(&app, cell)
            .filled
            .and_then(|filled| filled.cooldown),
        Some("3".to_string())
    );
    let world = app.world_mut();
    assert_eq!(
        world.query::<&Sweep>().iter(world).count(),
        1,
        "the cooldown wedge is drawn"
    );
    assert_eq!(use_slot(&mut app), Some(FxKind::Refuse));
    assert_eq!(
        notice(&app),
        Some(Notice::Refused("#5 is ready in 3s".to_string()))
    );
}

#[test]
fn the_cast_bar_follows_the_heros_cast_only() {
    let mut app = drawn_app();
    let cast = |src_id| SkillCastStarted {
        src_id,
        target_id: 0,
        x: 0,
        y: 0,
        skill_id: 5,
        property: 0,
        cast_time: 2000,
    };
    let casting = |app: &App| {
        let ui = app.world().resource::<HotbarUi>();
        ui.casting.as_ref().map(|casting| casting.skill_id)
    };
    let bars = |app: &mut App| {
        let world = app.world_mut();
        world.query::<&CastFill>().iter(world).count()
    };
    app.insert_resource(tree(vec![(5, node(7, 15, CAST))]));
    hotbar(&mut app).assign(0, HotbarSlot::Skill(5));

    app.world_mut().write_message(cast(9));
    app.update();
    assert_eq!(casting(&app), None, "someone else's cast");
    assert_eq!(bars(&mut app), 0);

    app.world_mut().write_message(cast(2_000_001));
    app.update();
    assert_eq!(casting(&app), Some(5));
    assert_eq!(bars(&mut app), 1, "the slot being cast grows a bar");

    app.world_mut()
        .write_message(CastCancelled { gid: 2_000_001 });
    app.update();
    assert_eq!(casting(&app), None);
    assert_eq!(bars(&mut app), 0);
}

#[test]
fn a_timed_status_from_a_skill_lights_its_slot() {
    let mut app = drawn_app();
    app.insert_resource(tree(vec![(5, node(7, 15, CAST))]));
    hotbar(&mut app).assign(0, HotbarSlot::Skill(5));
    app.world_mut()
        .resource_mut::<LocalStatuses>()
        .active
        .insert(
            12,
            ActiveStatus {
                total_ms: 180_000,
                expires_at: Some(Duration::from_secs(3600)),
                permanent: false,
                source_skill_id: Some(5),
            },
        );
    app.update();

    let cell = cell_at(&mut app, 0, 0);
    let filled = shown_view(&app, cell).filled.unwrap();
    assert!(filled.buff.is_some());
    assert_eq!(
        app.world().get::<Drawn>(cell).unwrap().edge,
        Some(Edge::Active)
    );
}

#[test]
fn hotbar_resources_are_registered() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(HotbarWidgetPlugin);
    assert!(app.world().contains_resource::<HotbarDrag>());
    assert!(app.world().contains_resource::<HotbarUi>());
}
