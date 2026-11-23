use std::collections::BTreeMap;
use std::time::Duration;

use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use bevy::time::TimeUpdateStrategy;
use bevy::ui_widgets::Activate;
use leafwing_input_manager::prelude::ActionState;
use lifthrasir_assets::catalogs::QuestDb;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::input::{PlayerAction, UiFocus};
use lifthrasir_client::domain::quest::{ObjectiveCompleted, QuestLog, QuestPins};
use lifthrasir_data::{QuestData, QuestText};
use net_contract::commands::{NavigationRequested, QuestCommand};
use net_contract::dto::{
    HuntFilter, NavigationTarget, QuestEntry, QuestGiver, QuestObjective, QuestState,
};
use net_contract::events::QuestEvent;

use super::view::*;
use super::{Act, LedgerTab, LedgerUi, on_button, spawn_window, toggle_key};

fn hunt(name: &str, needed: u32, current: u32) -> QuestObjective {
    QuestObjective {
        mob_id: 1002,
        mob_name: name.into(),
        needed,
        current,
        filter: HuntFilter::default(),
    }
}

fn giver(name: &str) -> QuestGiver {
    QuestGiver {
        name: name.into(),
        map: "prontera".into(),
        x: 150,
        y: 180,
    }
}

fn entry(id: u32, state: QuestState, objectives: Vec<QuestObjective>) -> QuestEntry {
    QuestEntry {
        quest_id: id,
        state,
        title: String::new(),
        objectives,
        deadline: None,
        giver: Some(giver("Guard")),
    }
}

fn log(entries: Vec<QuestEntry>) -> QuestLog {
    let mut log = QuestLog::default();
    log.apply(&QuestEvent::List(entries));
    log
}

fn db(quests: &[(u32, &str, &[&str], &str)]) -> QuestDb {
    let quests: BTreeMap<u32, QuestText> = quests
        .iter()
        .map(|(id, title, lines, summary)| {
            (
                *id,
                QuestText {
                    title: (*title).into(),
                    description: lines.iter().map(|l| (*l).into()).collect(),
                    summary: (*summary).into(),
                },
            )
        })
        .collect();
    QuestDb::from_data(QuestData { quests })
}

fn ledger(tab: LedgerTab, selected: Option<u32>, query: &str) -> LedgerUi {
    LedgerUi {
        tab,
        selected,
        query: query.into(),
        ..Default::default()
    }
}

fn mixed() -> (QuestLog, QuestDb) {
    let log = log(vec![
        entry(1, QuestState::Active, vec![hunt("Poring", 10, 3)]),
        entry(2, QuestState::Active, vec![hunt("Lunatic", 5, 5)]),
        entry(3, QuestState::Inactive, vec![hunt("Fabre", 4, 0)]),
        entry(4, QuestState::Complete, vec![hunt("Drops", 4, 4)]),
        entry(5, QuestState::Active, vec![hunt("Pupa", 2, 2)]),
    ]);
    let db = db(&[
        (
            1,
            "Porings",
            &["Kill ^ff0000porings^000000."],
            "Hunt porings",
        ),
        (2, "Lunatics", &["Line one", "Line two"], ""),
        (3, "Fabres", &[], "Hunt fabres"),
        (4, "Drops", &[], ""),
        (5, "Pupas", &[], ""),
    ]);
    (log, db)
}

#[test]
fn tabs_count_each_state_and_headline_counts_ready() {
    let (log, db) = mixed();
    let view = ledger_view(&log, &db, &QuestPins::default(), &LedgerUi::default(), 0);
    let counts: Vec<u32> = view.tabs.iter().map(|t| t.count).collect();
    assert_eq!(counts, [3, 1, 1]);
    assert_eq!(view.headline, "3 active · 2 ready to turn in");
    assert_eq!(view.pin_note, "0 of 3 tracked on screen");
}

#[test]
fn active_tab_groups_ready_before_in_progress_in_id_order() {
    let (log, db) = mixed();
    let view = ledger_view(&log, &db, &QuestPins::default(), &LedgerUi::default(), 0);
    let groups: Vec<(Option<&str>, Vec<u32>)> = view
        .groups
        .iter()
        .map(|g| (g.label, g.rows.iter().map(|r| r.id).collect()))
        .collect();
    assert_eq!(
        groups,
        [
            (Some("READY TO TURN IN"), vec![2, 5]),
            (Some("IN PROGRESS"), vec![1])
        ]
    );
}

#[test]
fn shelved_and_completed_are_single_ungrouped_lists() {
    let (log, db) = mixed();
    let pins = QuestPins::default();
    let shelved = ledger_view(&log, &db, &pins, &ledger(LedgerTab::Shelved, None, ""), 0);
    assert_eq!(shelved.groups.len(), 1);
    assert_eq!(shelved.groups[0].label, None);
    assert_eq!(shelved.groups[0].rows[0].id, 3);
    let done = ledger_view(&log, &db, &pins, &ledger(LedgerTab::Completed, None, ""), 0);
    assert_eq!(done.groups[0].rows[0].id, 4);
    assert_eq!(done.groups[0].rows[0].chip, None);
}

#[test]
fn search_matches_title_giver_name_and_map_ignoring_case() {
    let (log, db) = mixed();
    let pins = QuestPins::default();
    let ids = |q: &str| -> Vec<u32> {
        ledger_view(&log, &db, &pins, &ledger(LedgerTab::Active, None, q), 0)
            .groups
            .iter()
            .flat_map(|g| g.rows.iter().map(|r| r.id))
            .collect()
    };
    assert_eq!(ids("PORING"), [1]);
    assert_eq!(ids("guard").len(), 3);
    assert_eq!(ids("PRONTERA").len(), 3);
    assert!(ids("nothing").is_empty());
}

#[test]
fn selection_cleared_when_quest_leaves_log() {
    let (log, db) = mixed();
    let pins = QuestPins::default();
    let missing = ledger_view(
        &log,
        &db,
        &pins,
        &ledger(LedgerTab::Active, Some(99), ""),
        0,
    );
    assert_eq!(missing.detail, None);
    let other_tab = ledger_view(&log, &db, &pins, &ledger(LedgerTab::Active, Some(3), ""), 0);
    assert_eq!(other_tab.detail, None);
    let ok = ledger_view(&log, &db, &pins, &ledger(LedgerTab::Active, Some(1), ""), 0);
    assert!(ok.detail.is_some());
    assert!(ok.groups.iter().flat_map(|g| &g.rows).any(|r| r.selected));
}

#[test]
fn countdown_after_deadline_reads_expired() {
    assert_eq!(countdown(1000, 1000), "Expired");
    assert_eq!(countdown(1000, 1060), "Expired");
    assert_eq!(countdown(11_520, 0), "3 h 12 m");
    assert_eq!(countdown(2_700, 0), "45 m");
    assert_eq!(countdown(30, 0), "1 m");
    assert_eq!(countdown(3_599, 0), "1 h 0 m");
    assert_eq!(countdown(3_540, 0), "59 m");
    assert_eq!(countdown(187_200, 0), "2 d 4 h");
}

#[test]
fn uncatalogued_quest_uses_server_title() {
    let mut named = entry(7, QuestState::Active, vec![]);
    named.title = "Server Title".into();
    let nameless = entry(8, QuestState::Active, vec![]);
    let log = log(vec![named, nameless]);
    let db = db(&[]);
    let pins = QuestPins::default();
    let detail = |id| {
        ledger_view(
            &log,
            &db,
            &pins,
            &ledger(LedgerTab::Active, Some(id), ""),
            0,
        )
        .detail
        .unwrap()
    };
    let named = detail(7);
    assert_eq!(named.title, "Server Title");
    assert!(named.story.is_empty());
    assert!(named.steps.is_empty());
    assert_eq!(detail(8).title, "Quest #8");
}

#[test]
fn step_less_catalogued_quest_shows_summary_or_first_line() {
    let log = log(vec![
        entry(1, QuestState::Active, vec![]),
        entry(2, QuestState::Active, vec![]),
    ]);
    let db = db(&[
        (1, "A", &["First", "Second"], "Sum"),
        (2, "B", &["First", "Second"], ""),
    ]);
    let pins = QuestPins::default();
    let step = |id| {
        let d = ledger_view(
            &log,
            &db,
            &pins,
            &ledger(LedgerTab::Active, Some(id), ""),
            0,
        )
        .detail
        .unwrap();
        assert_eq!(d.story, ["First", "Second"]);
        d.steps[0].label.clone()
    };
    assert_eq!(step(1), "Sum");
    assert_eq!(step(2), "First");
}

#[test]
fn objective_labels() {
    let mut o = hunt("Zerom", 1, 0);
    assert_eq!(objective_label(&o), "Defeat Zerom");
    o.mob_name.clear();
    o.filter.race = "demihuman".into();
    o.filter.min_level = 50;
    o.filter.max_level = 60;
    assert_eq!(objective_label(&o), "Defeat Demi-Human monsters Lv 50-60");
    o.filter.map = "prt_fild08".into();
    assert_eq!(
        objective_label(&o),
        "Defeat Demi-Human monsters Lv 50-60 in prt_fild08"
    );
    assert_eq!(objective_label(&hunt("", 1, 0)), "Defeat monsters");
}

#[test]
fn chips_and_footer_follow_state() {
    let mut timed = entry(1, QuestState::Active, vec![hunt("Poring", 10, 3)]);
    timed.deadline = Some(11_520);
    let mut ready_timed = entry(2, QuestState::Active, vec![hunt("Lunatic", 5, 5)]);
    ready_timed.deadline = Some(11_520);
    let mut shelved = entry(3, QuestState::Inactive, vec![hunt("Fabre", 4, 0)]);
    shelved.deadline = Some(11_520);
    let mut nogiver = entry(4, QuestState::Active, vec![hunt("Pupa", 2, 2)]);
    nogiver.giver = None;
    let mut anon = entry(5, QuestState::Active, vec![hunt("Pupa", 2, 2)]);
    anon.giver = Some(giver(""));
    let done = entry(6, QuestState::Complete, vec![hunt("Drops", 4, 4)]);
    let log = log(vec![timed, ready_timed, shelved, nogiver, anon, done]);
    let db = db(&[]);
    let pins = QuestPins::default();
    let view = |tab, id| ledger_view(&log, &db, &pins, &ledger(tab, Some(id), ""), 0);
    let chip = |tab, id| {
        view(tab, id)
            .groups
            .iter()
            .flat_map(|g| &g.rows)
            .find(|r| r.id == id)
            .unwrap()
            .chip
            .clone()
    };
    assert_eq!(
        chip(LedgerTab::Active, 1),
        Some(Chip::Ends("Ends 3 h 12 m".into()))
    );
    assert_eq!(chip(LedgerTab::Active, 2), Some(Chip::Ready));
    assert_eq!(chip(LedgerTab::Shelved, 3), None);

    let d1 = view(LedgerTab::Active, 1).detail.unwrap();
    assert_eq!(d1.shelve, Some(ShelveAction::Shelve));
    assert_eq!(d1.ready_text, None);
    assert_eq!(
        d1.giver_line.as_deref(),
        Some("Guard · prontera (150, 180)")
    );
    assert_eq!(d1.timer.as_deref(), Some("3 h 12 m"));
    assert!(d1.can_navigate);
    let d2 = view(LedgerTab::Active, 2).detail.unwrap();
    assert_eq!(d2.ready_text.as_deref(), Some("Ready · Return to Guard"));
    let d3 = view(LedgerTab::Shelved, 3).detail.unwrap();
    assert_eq!(d3.shelve, Some(ShelveAction::Resume));
    assert_eq!(d3.timer, None);
    let d4 = view(LedgerTab::Active, 4).detail.unwrap();
    assert!(!d4.can_navigate);
    assert_eq!(d4.giver_line, None);
    assert_eq!(d4.ready_text.as_deref(), Some("Ready to turn in"));
    let d5 = view(LedgerTab::Active, 5).detail.unwrap();
    assert_eq!(d5.giver_line.as_deref(), Some("prontera (150, 180)"));
    assert_eq!(d5.ready_text.as_deref(), Some("Ready · Return to prontera"));
    let d6 = view(LedgerTab::Completed, 6).detail.unwrap();
    assert_eq!(d6.shelve, None);
}

#[test]
fn row_sub_line_and_progress() {
    let log = log(vec![entry(
        1,
        QuestState::Active,
        vec![hunt("A", 4, 4), hunt("B", 4, 0)],
    )]);
    let view = ledger_view(
        &log,
        &db(&[]),
        &QuestPins::default(),
        &LedgerUi::default(),
        0,
    );
    let row = &view.groups[0].rows[0];
    assert_eq!(row.sub_line, "Guard · 1 of 2 done");
    assert_eq!(row.bar_pct, 50);
}

#[test]
fn tracker_lists_pins_in_order_and_skips_stale_ones() {
    let full = log(vec![
        entry(1, QuestState::Active, vec![hunt("Poring", 10, 3)]),
        entry(2, QuestState::Active, vec![hunt("Lunatic", 5, 5)]),
        entry(3, QuestState::Active, vec![hunt("Fabre", 4, 0)]),
    ]);
    let db = db(&[(1, "Porings", &[], ""), (2, "Lunatics", &[], "")]);
    let mut pins = QuestPins::default();
    pins.toggle(2, &full).unwrap();
    pins.toggle(3, &full).unwrap();
    pins.toggle(1, &full).unwrap();

    let view = tracker_view(&full, &db, &pins, 0);
    let ids: Vec<u32> = view.quests.iter().map(|q| q.id).collect();
    assert_eq!(ids, [2, 3, 1]);
    assert_eq!(view.quests[0].title, "Lunatics");
    assert_eq!(
        view.quests[0].ready_text.as_deref(),
        Some("Ready · Return to Guard")
    );
    assert_eq!(view.quests[2].steps[0].label, "Defeat Poring");
    assert_eq!(view.quests[2].steps[0].count_text, "3/10");

    let later = log(vec![
        entry(2, QuestState::Active, vec![hunt("Lunatic", 5, 5)]),
        entry(3, QuestState::Inactive, vec![hunt("Fabre", 4, 0)]),
    ]);
    let ids: Vec<u32> = tracker_view(&later, &db, &pins, 0)
        .quests
        .iter()
        .map(|q| q.id)
        .collect();
    assert_eq!(ids, [2]);
}

/// An app holding the ledger window under a bare HUD root, over `log`.
fn window_app(log: QuestLog, ui: LedgerUi) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    let (_, db) = mixed();
    app.insert_resource(log)
        .insert_resource(db)
        .insert_resource(ui)
        .init_resource::<QuestPins>()
        .add_message::<QuestCommand>()
        .add_message::<NavigationRequested>()
        .add_systems(Update, (toggle_key, super::rebuild).chain());
    let root = app.world_mut().spawn_empty().id();
    let mut commands = app.world_mut().commands();
    spawn_window(&mut commands, root);
    app.world_mut().flush();
    app
}

fn press(app: &mut App, act: Act) {
    let button = app.world_mut().spawn(act).observe(on_button).id();
    app.world_mut().trigger(Activate { entity: button });
    app.world_mut().flush();
}

fn sent<M: Message + Clone>(app: &App) -> Vec<M> {
    let messages = app.world().resource::<Messages<M>>();
    messages.get_cursor().read(messages).cloned().collect()
}

fn selected(id: u32) -> LedgerUi {
    LedgerUi {
        open: true,
        selected: Some(id),
        ..Default::default()
    }
}

#[test]
fn shelve_asks_the_server_and_leaves_the_log_alone() {
    let (log, _) = mixed();
    let mut app = window_app(log, selected(1));
    press(&mut app, Act::Shelve);
    assert_eq!(
        sent::<QuestCommand>(&app),
        [QuestCommand::SetActive {
            quest_id: 1,
            active: false
        }]
    );
    let log = app.world().resource::<QuestLog>();
    assert_eq!(log.get(1).unwrap().state, QuestState::Active);
}

#[test]
fn resume_asks_the_server_to_activate_a_shelved_quest() {
    let (log, _) = mixed();
    let ui = LedgerUi {
        tab: LedgerTab::Shelved,
        ..selected(3)
    };
    let mut app = window_app(log, ui);
    press(&mut app, Act::Resume);
    assert_eq!(
        sent::<QuestCommand>(&app),
        [QuestCommand::SetActive {
            quest_id: 3,
            active: true
        }]
    );
}

#[test]
fn navigate_routes_to_the_giver() {
    let (log, _) = mixed();
    let mut app = window_app(log, selected(1));
    press(&mut app, Act::Navigate);
    let sent = sent::<NavigationRequested>(&app);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].target,
        NavigationTarget::Coord {
            map: "prontera".into(),
            x: 150,
            y: 180
        }
    );
    assert_eq!((sent[0].flag, sent[0].hide_window), (0, false));
}

#[test]
fn a_fourth_pin_shows_the_toast_and_keeps_the_pins() {
    let full = log(vec![
        entry(1, QuestState::Active, vec![hunt("Poring", 10, 3)]),
        entry(2, QuestState::Active, vec![hunt("Lunatic", 5, 0)]),
        entry(3, QuestState::Active, vec![hunt("Fabre", 4, 0)]),
        entry(4, QuestState::Active, vec![hunt("Pupa", 2, 0)]),
    ]);
    let mut pins = QuestPins::default();
    for id in [1, 2, 3] {
        pins.toggle(id, &full).unwrap();
    }
    let mut app = window_app(full, selected(1));
    app.insert_resource(pins);
    press(&mut app, Act::Pin(4));
    assert_eq!(app.world().resource::<QuestPins>().ids(), [1, 2, 3]);
    let toast = app.world().resource::<LedgerUi>().toast.clone();
    assert_eq!(
        toast.map(|(text, _)| text).as_deref(),
        Some("You can track up to 3 quests on screen.")
    );
}

#[test]
fn the_quest_ledger_key_toggles_the_window() {
    let (log, _) = mixed();
    let mut app = window_app(log, LedgerUi::default());
    let player = app
        .world_mut()
        .spawn((LocalPlayer, ActionState::<PlayerAction>::default()))
        .id();
    let set = |app: &mut App, down: bool| {
        let mut player = app.world_mut().entity_mut(player);
        let mut actions = player.get_mut::<ActionState<PlayerAction>>().unwrap();
        match down {
            true => actions.press(&PlayerAction::QuestLedger),
            false => actions.release(&PlayerAction::QuestLedger),
        }
    };
    set(&mut app, true);
    app.update();
    assert!(app.world().resource::<LedgerUi>().open);
    set(&mut app, false);
    app.update();
    set(&mut app, true);
    app.update();
    assert!(!app.world().resource::<LedgerUi>().open);
}

fn rows(app: &mut App) -> Vec<(Entity, u32)> {
    let world = app.world_mut();
    let mut rows: Vec<(Entity, u32)> = world
        .query::<(Entity, &Act)>()
        .iter(world)
        .filter_map(|(entity, act)| match act {
            Act::Select(id) => Some((entity, *id)),
            _ => None,
        })
        .collect();
    rows.sort_by_key(|(_, id)| *id);
    rows
}

#[test]
fn the_picked_quest_fills_the_page_footer_and_toast() {
    let mut timed = entry(1, QuestState::Active, vec![hunt("Poring", 10, 3)]);
    timed.deadline = Some(u64::MAX);
    let ui = LedgerUi {
        toast: Some(("Careful".into(), 2.6)),
        ..selected(1)
    };
    let mut app = window_app(log(vec![timed]), ui);
    app.update();
    app.update();
    let world = app.world_mut();
    let texts: Vec<String> = world
        .query::<&Text>()
        .iter(world)
        .map(|text| text.0.clone())
        .collect();
    for shown in ["Porings", "Kill ", "Navigate", "Shelve", "Careful"] {
        assert!(texts.iter().any(|text| text == shown), "missing {shown:?}");
    }
    let red_run = world
        .query::<(&TextSpan, &TextColor)>()
        .iter(world)
        .any(|(span, color)| span.0 == "porings" && color.0 == Color::srgb_u8(0xff, 0, 0));
    assert!(red_run, "the story keeps its colour codes");
    let timers = texts.iter().filter(|t| t.starts_with("Ends ")).count();
    assert_eq!(timers, 2, "the row chip and the page deadline");
}

#[test]
fn the_list_holds_one_row_per_quest_and_keeps_them_while_unchanged() {
    let (log, _) = mixed();
    let ui = LedgerUi {
        open: true,
        ..Default::default()
    };
    let mut app = window_app(log, ui);
    app.update();
    app.update();
    let first = rows(&mut app);
    let ids: Vec<u32> = first.iter().map(|(_, id)| *id).collect();
    assert_eq!(ids, [1, 2, 5]);

    app.update();
    assert_eq!(rows(&mut app), first);

    app.world_mut().resource_mut::<LedgerUi>().tab = LedgerTab::Shelved;
    app.update();
    app.update();
    let ids: Vec<u32> = rows(&mut app).iter().map(|(_, id)| *id).collect();
    assert_eq!(ids, [3]);
}

/// A tracker over `pinned` quests of `log`, with the ledger window closed.
fn tracker_app(log: QuestLog, pinned: &[u32], ui: LedgerUi) -> App {
    let mut pins = QuestPins::default();
    for id in pinned {
        pins.toggle(*id, &log).unwrap();
    }
    let mut app = window_app(log, ui);
    app.insert_resource(pins)
        .add_message::<ObjectiveCompleted>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs(1)))
        .add_systems(
            Update,
            (super::expire_messages, super::show_banner)
                .chain()
                .before(super::rebuild),
        );
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .set_max_delta(Duration::from_secs(10));
    app.update();
    app.update();
    app
}

fn texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world
        .query::<&Text>()
        .iter(world)
        .map(|t| t.0.clone())
        .collect()
}

fn control(app: &mut App, wanted: Act) -> Option<Entity> {
    let world = app.world_mut();
    world
        .query::<(Entity, &Act)>()
        .iter(world)
        .find_map(|(entity, act)| (*act == wanted).then_some(entity))
}

fn activate(app: &mut App, entity: Entity) {
    app.world_mut().trigger(Activate { entity });
    app.world_mut().flush();
    app.update();
}

#[test]
fn clicking_a_tracker_row_opens_the_ledger_on_that_quest() {
    let (log, _) = mixed();
    let ui = LedgerUi {
        tab: LedgerTab::Completed,
        ..Default::default()
    };
    let mut app = tracker_app(log, &[1, 2], ui);
    let row = control(&mut app, Act::Open(2)).expect("a row for the pinned quest");
    activate(&mut app, row);
    let ui = app.world().resource::<LedgerUi>();
    assert!(ui.open);
    assert_eq!(ui.selected, Some(2));
    assert_eq!(ui.tab, LedgerTab::Active);
}

#[test]
fn a_finished_objective_raises_a_banner_for_three_seconds() {
    let (log, _) = mixed();
    let mut app = tracker_app(log, &[1], LedgerUi::default());
    app.world_mut().write_message(ObjectiveCompleted {
        quest_id: 2,
        index: 0,
    });
    app.update();
    app.update();
    let shown = texts(&mut app);
    assert!(shown.iter().any(|t| t == "OBJECTIVE COMPLETE"));
    assert!(shown.iter().any(|t| t == "Defeat Lunatic · 5 / 5"));

    for _ in 0..3 {
        app.update();
    }
    assert!(!texts(&mut app).iter().any(|t| t == "OBJECTIVE COMPLETE"));
}

#[test]
fn the_chevron_hides_and_shows_the_list() {
    let (log, _) = mixed();
    let mut app = tracker_app(log, &[1], LedgerUi::default());
    assert!(control(&mut app, Act::Open(1)).is_some());

    let chevron = control(&mut app, Act::ToggleTracker).unwrap();
    activate(&mut app, chevron);
    app.update();
    assert!(control(&mut app, Act::Open(1)).is_none());
    assert!(texts(&mut app).iter().any(|t| t == "1 tracked"));

    let chevron = control(&mut app, Act::ToggleTracker).unwrap();
    activate(&mut app, chevron);
    app.update();
    assert!(control(&mut app, Act::Open(1)).is_some());
}

#[test]
fn without_pins_the_plate_is_only_its_header() {
    let (log, _) = mixed();
    let mut app = tracker_app(log, &[], LedgerUi::default());
    let shown = texts(&mut app);
    assert!(shown.iter().any(|t| t == "0 tracked"));
    assert!(!shown.iter().any(|t| t.starts_with("Defeat")));
    assert!(control(&mut app, Act::Open(1)).is_none());
}

#[test]
fn ledger_state_survives_a_warp_and_resets_at_character_selection() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .init_state::<GameState>()
        .init_resource::<UiFocus>()
        .add_plugins(super::QuestLedgerPlugin);
    let set = |app: &mut App, state: GameState| {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(state);
        app.update();
        app.update();
    };
    set(&mut app, GameState::InGame);
    {
        let mut ui = app.world_mut().resource_mut::<LedgerUi>();
        ui.open = true;
        ui.tracker_collapsed = true;
    }

    set(&mut app, GameState::Loading);
    set(&mut app, GameState::InGame);
    let ui = app.world().resource::<LedgerUi>();
    assert!(ui.open && ui.tracker_collapsed);

    set(&mut app, GameState::CharacterSelection);
    let ui = app.world().resource::<LedgerUi>();
    assert!(!ui.open && !ui.tracker_collapsed);
}
