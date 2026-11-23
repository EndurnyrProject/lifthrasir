use std::path::PathBuf;

use bevy::ecs::system::RunSystemOnce;
use net_contract::dto::{HuntFilter, QuestObjective};
use net_contract::state::ZoneSession;

use super::persistence::{
    PinsPersistState, load_quest_pins, persist_quest_pins, save_quest_pins_on_exit,
};
use super::*;

fn objective(current: u32, needed: u32) -> QuestObjective {
    QuestObjective {
        mob_id: 1002,
        mob_name: "Poring".into(),
        needed,
        current,
        filter: HuntFilter::default(),
    }
}

fn quest(quest_id: u32, state: QuestState, objectives: Vec<QuestObjective>) -> QuestEntry {
    QuestEntry {
        quest_id,
        state,
        title: format!("Quest {quest_id}"),
        objectives,
        deadline: None,
        giver: None,
    }
}

fn log_of(entries: impl IntoIterator<Item = QuestEntry>) -> QuestLog {
    let mut log = QuestLog::default();
    log.apply(&QuestEvent::List(entries.into_iter().collect()));
    log
}

fn ids(log: &QuestLog) -> Vec<u32> {
    log.iter().map(|entry| entry.quest_id).collect()
}

fn progress(quest_id: u32, objective_index: usize, count: u32, needed: u32) -> QuestEvent {
    QuestEvent::HuntProgress {
        quest_id,
        objective_index,
        count,
        needed,
    }
}

#[test]
fn a_list_replaces_the_log_and_added_and_removed_edit_it() {
    let mut log = log_of([
        quest(1, QuestState::Active, vec![]),
        quest(2, QuestState::Active, vec![]),
    ]);
    assert_eq!(ids(&log), [1, 2]);

    log.apply(&QuestEvent::List(vec![quest(
        3,
        QuestState::Active,
        vec![],
    )]));
    assert_eq!(ids(&log), [3]);

    log.apply(&QuestEvent::Added(quest(1, QuestState::Active, vec![])));
    assert_eq!(
        ids(&log),
        [1, 3],
        "quest id order, whatever the arrival order"
    );

    log.apply(&QuestEvent::Removed { quest_id: 3 });
    log.apply(&QuestEvent::Removed { quest_id: 77 });
    assert_eq!(ids(&log), [1]);
    assert!(log.get(3).is_none());
}

#[test]
fn state_changes_and_hunt_progress_update_the_entry() {
    let mut log = log_of([quest(1, QuestState::Active, vec![objective(0, 10)])]);

    log.apply(&QuestEvent::StateChanged {
        quest_id: 1,
        state: QuestState::Inactive,
    });
    log.apply(&progress(1, 0, 4, 12));

    let entry = log.get(1).expect("the quest is still logged");
    assert_eq!(entry.state, QuestState::Inactive);
    assert_eq!(
        (entry.objectives[0].current, entry.objectives[0].needed),
        (4, 12)
    );
}

#[test]
fn hunt_progress_for_an_unknown_quest_or_objective_is_ignored() {
    let mut log = log_of([quest(1, QuestState::Active, vec![objective(0, 10)])]);
    let before = log.get(1).cloned();

    assert_eq!(log.apply(&progress(9, 0, 10, 10)), None);
    assert_eq!(log.apply(&progress(1, 5, 10, 10)), None);

    assert_eq!(log.get(1).cloned(), before);
    assert_eq!(ids(&log), [1]);
}

#[test]
fn objective_completed_fires_once() {
    let mut log = log_of([quest(1, QuestState::Active, vec![objective(8, 10)])]);

    assert_eq!(log.apply(&progress(1, 0, 9, 10)), None);
    assert_eq!(
        log.apply(&progress(1, 0, 10, 10)),
        Some(ObjectiveCompleted {
            quest_id: 1,
            index: 0
        })
    );
    assert_eq!(
        log.apply(&progress(1, 0, 10, 10)),
        None,
        "a repeat at the cap"
    );
    assert_eq!(
        log.apply(&progress(1, 0, 11, 10)),
        None,
        "a kill past the cap"
    );
}

#[test]
fn a_quest_is_ready_when_active_and_every_hunt_is_met() {
    let met = || vec![objective(10, 10), objective(12, 10)];
    let short = vec![objective(10, 10), objective(9, 10)];

    assert!(is_ready(&quest(1, QuestState::Active, met())));
    assert!(!is_ready(&quest(1, QuestState::Active, short)));
    assert!(!is_ready(&quest(1, QuestState::Active, vec![])));
    assert!(!is_ready(&quest(1, QuestState::Inactive, met())));
    assert!(!is_ready(&quest(1, QuestState::Complete, met())));
}

#[test]
fn pinning_stops_at_three_active_quests_and_a_pin_can_be_taken_back() {
    let log = log_of([
        quest(1, QuestState::Active, vec![]),
        quest(2, QuestState::Active, vec![]),
        quest(3, QuestState::Active, vec![]),
        quest(4, QuestState::Active, vec![]),
        quest(5, QuestState::Inactive, vec![]),
    ]);
    let mut pins = QuestPins::default();

    for quest_id in [1, 2, 3] {
        assert_eq!(pins.toggle(quest_id, &log), Ok(()));
    }
    assert_eq!(pins.toggle(4, &log), Err(PinRefused::Full));
    assert_eq!(pins.ids(), [1, 2, 3]);

    assert_eq!(pins.toggle(2, &log), Ok(()), "unpinning works while full");
    assert!(!pins.contains(2));
    assert_eq!(pins.toggle(5, &log), Err(PinRefused::NotActive), "shelved");
    assert_eq!(pins.toggle(99, &log), Err(PinRefused::NotActive), "unknown");
    assert_eq!(pins.toggle(4, &log), Ok(()));
    assert_eq!(pins.ids(), [1, 3, 4]);
}

#[test]
fn prune_drops_missing_and_inactive_pins() {
    let mut pins = QuestPins::default();
    let active = log_of([
        quest(1, QuestState::Active, vec![]),
        quest(2, QuestState::Active, vec![]),
        quest(3, QuestState::Active, vec![]),
    ]);
    for quest_id in [1, 2, 3] {
        pins.toggle(quest_id, &active).unwrap();
    }

    let relisted = log_of([
        quest(2, QuestState::Inactive, vec![]),
        quest(3, QuestState::Active, vec![]),
    ]);
    pins.prune(&relisted);

    assert_eq!(pins.ids(), [3], "1 is gone from the log and 2 is shelved");
}

#[test]
fn prune_keeps_every_pin_until_the_first_list_arrives() {
    let all = log_of((1..=2).map(|id| quest(id, QuestState::Active, vec![])));
    let mut pins = QuestPins::default();
    pins.toggle(1, &all).unwrap();
    pins.toggle(2, &all).unwrap();

    let mut log = QuestLog::default();
    pins.prune(&log);
    log.apply(&QuestEvent::Added(quest(1, QuestState::Active, vec![])));
    pins.prune(&log);
    assert!(!log.synced());
    assert_eq!(pins.ids(), [1, 2]);

    log.apply(&QuestEvent::List(vec![]));
    pins.prune(&log);
    assert!(log.synced());
    assert!(pins.ids().is_empty(), "a list without them takes the pins");
}

#[test]
fn prune_cuts_a_hand_edited_file_down_to_the_cap() {
    let log = log_of((1..=4).map(|id| quest(id, QuestState::Active, vec![])));
    let mut pins: QuestPins = ron::from_str("(ids:[1,2,3,4])").unwrap();

    pins.prune(&log);

    assert_eq!(pins.ids(), [1, 2, 3]);
}

#[test]
fn replaying_events_announces_completions_then_prunes_pins() {
    let log = log_of([
        quest(1, QuestState::Active, vec![objective(9, 10)]),
        quest(2, QuestState::Active, vec![]),
    ]);
    let mut pins = QuestPins::default();
    pins.toggle(1, &log).unwrap();
    pins.toggle(2, &log).unwrap();
    let mut app = App::new();
    app.add_message::<QuestEvent>()
        .add_message::<ObjectiveCompleted>()
        .insert_resource(log)
        .insert_resource(pins)
        .add_systems(Update, replay_quests);
    app.world_mut().write_message(progress(1, 0, 10, 10));
    app.world_mut().write_message(QuestEvent::StateChanged {
        quest_id: 2,
        state: QuestState::Inactive,
    });

    app.update();

    let completed: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<ObjectiveCompleted>>()
        .drain()
        .collect();
    assert_eq!(
        completed,
        [ObjectiveCompleted {
            quest_id: 1,
            index: 0
        }]
    );
    assert_eq!(app.world().resource::<QuestPins>().ids(), [1]);
}

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Points the pins directory at a fresh temp dir for the duration of `f`, serialized
/// because the environment is process-global.
fn with_pins_dir<T>(name: &str, f: impl FnOnce() -> T) -> T {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir: PathBuf = std::env::temp_dir()
        .join("lifthrasir_quest_pins_test")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    // Serialized by ENV_LOCK; no other test reads this variable.
    unsafe { std::env::set_var("LIFTHRASIR_QUEST_PINS_DIR", &dir) };
    let result = f();
    unsafe { std::env::remove_var("LIFTHRASIR_QUEST_PINS_DIR") };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn session_app(char_id: u32) -> App {
    let mut app = App::new();
    app.add_message::<QuestEvent>()
        .add_message::<ObjectiveCompleted>()
        .init_resource::<QuestLog>()
        .init_resource::<QuestPins>()
        .init_resource::<PinsPersistState>()
        .insert_resource(ZoneSession {
            char_id,
            ..default()
        });
    app
}

fn pin_all(app: &mut App, quest_ids: &[u32]) {
    let log = log_of(
        quest_ids
            .iter()
            .map(|id| quest(*id, QuestState::Active, vec![])),
    );
    let mut pins = app.world_mut().resource_mut::<QuestPins>();
    for quest_id in quest_ids {
        pins.toggle(*quest_id, &log).unwrap();
    }
}

#[test]
fn pins_come_back_for_the_same_character_and_not_for_another() {
    with_pins_dir("round_trip", || {
        let mut app = session_app(5);
        app.add_systems(Update, (load_quest_pins, persist_quest_pins).chain());
        app.update();
        pin_all(&mut app, &[7, 9]);
        app.update();

        let mut same = session_app(5);
        same.add_systems(Update, load_quest_pins);
        same.update();
        assert_eq!(same.world().resource::<QuestPins>().ids(), [7, 9]);

        let mut other = session_app(6);
        other.add_systems(Update, load_quest_pins);
        other.update();
        assert!(other.world().resource::<QuestPins>().ids().is_empty());
    });
}

#[test]
fn a_pin_the_log_no_longer_holds_is_dropped_on_load_and_in_the_file() {
    with_pins_dir("stale_pin", || {
        let mut app = session_app(5);
        app.add_systems(Update, (load_quest_pins, persist_quest_pins).chain());
        app.update();
        pin_all(&mut app, &[7, 9]);
        app.update();

        let mut relog = session_app(5);
        relog.insert_resource(log_of([quest(9, QuestState::Active, vec![])]));
        relog.add_systems(Update, (load_quest_pins, persist_quest_pins).chain());
        relog.update();
        assert_eq!(relog.world().resource::<QuestPins>().ids(), [9]);

        let mut next = session_app(5);
        next.add_systems(Update, load_quest_pins);
        next.update();
        assert_eq!(next.world().resource::<QuestPins>().ids(), [9]);
    });
}

#[test]
fn pins_loaded_before_the_list_survive_until_it_arrives_and_are_pruned_then() {
    with_pins_dir("list_after_load", || {
        let mut app = session_app(5);
        app.add_systems(Update, (load_quest_pins, persist_quest_pins).chain());
        app.update();
        pin_all(&mut app, &[7, 9]);
        app.update();

        let mut relog = session_app(5);
        relog.add_systems(
            Update,
            (replay_quests, load_quest_pins, persist_quest_pins).chain(),
        );
        relog.update();
        assert_eq!(relog.world().resource::<QuestPins>().ids(), [7, 9]);

        relog.world_mut().write_message(QuestEvent::List(vec![quest(
            9,
            QuestState::Active,
            vec![],
        )]));
        relog.update();
        assert_eq!(relog.world().resource::<QuestPins>().ids(), [9]);

        let mut next = session_app(5);
        next.add_systems(Update, load_quest_pins);
        next.update();
        assert_eq!(next.world().resource::<QuestPins>().ids(), [9]);
    });
}

#[test]
fn leaving_the_game_saves_the_pins_and_lets_the_next_session_load_again() {
    with_pins_dir("on_exit", || {
        let mut app = session_app(5);
        app.add_systems(Update, load_quest_pins);
        app.update();
        pin_all(&mut app, &[4]);
        app.world_mut()
            .run_system_once(save_quest_pins_on_exit)
            .unwrap();

        app.world_mut().insert_resource(QuestPins::default());
        app.update();

        assert_eq!(app.world().resource::<QuestPins>().ids(), [4]);
    });
}
