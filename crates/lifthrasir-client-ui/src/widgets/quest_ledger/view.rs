//! Plain views of the quest ledger and the tracker plate: what each region shows,
//! read from the quest log, the text catalog, the pins and the window state.
//! Nothing here touches the ECS.

use lifthrasir_assets::catalogs::QuestDb;
use lifthrasir_client::domain::quest::{MAX_PINS, QuestLog, QuestPins, is_ready};
use net_contract::dto::{HuntFilter, QuestEntry, QuestGiver, QuestObjective, QuestState};

use super::{LedgerTab, LedgerUi};

const DAY: u64 = 86_400;
const HOUR: u64 = 3_600;
const MINUTE: u64 = 60;

#[derive(Clone, Debug, PartialEq)]
pub struct LedgerView {
    /// `"{a} active · {r} ready to turn in"`.
    pub headline: String,
    pub tabs: Vec<TabView>,
    pub groups: Vec<GroupView>,
    /// `None` shows the "Pick a quest from the ledger" state.
    pub detail: Option<DetailView>,
    /// `"{n} of 3 tracked on screen"`.
    pub pin_note: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TabView {
    pub tab: LedgerTab,
    pub label: &'static str,
    pub count: u32,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupView {
    /// `None` for the ungrouped Shelved and Completed lists.
    pub label: Option<&'static str>,
    pub rows: Vec<RowView>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Chip {
    Ready,
    /// The full chip text, e.g. `"Ends 3 h 12 m"`.
    Ends(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RowView {
    pub id: u32,
    pub title: String,
    pub sub_line: String,
    pub chip: Option<Chip>,
    pub bar_pct: u8,
    pub pinned: bool,
    pub selected: bool,
    pub ready: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StepView {
    pub label: String,
    /// `"{current}/{needed}"`, empty for a step that is only text.
    pub count_text: String,
    pub bar_pct: u8,
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DetailView {
    pub id: u32,
    pub title: String,
    pub giver_line: Option<String>,
    /// The countdown of an active quest with a deadline.
    pub timer: Option<String>,
    /// Description lines with their `^RRGGBB` codes intact; the scene colors them.
    pub story: Vec<String>,
    pub steps: Vec<StepView>,
    pub can_navigate: bool,
    /// Shelve on Active, resume on Shelved, none on Completed.
    pub shelve: Option<ShelveAction>,
    pub ready_text: Option<String>,
}

/// The footer button that moves a quest in or out of the shelf.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShelveAction {
    Shelve,
    Resume,
}

impl ShelveAction {
    pub fn label(self) -> &'static str {
        match self {
            ShelveAction::Shelve => "Shelve",
            ShelveAction::Resume => "Resume",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrackerView {
    pub quests: Vec<TrackerQuest>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrackerQuest {
    pub id: u32,
    pub title: String,
    pub steps: Vec<StepView>,
    pub ready_text: Option<String>,
    pub timer: Option<String>,
}

pub fn ledger_view(
    log: &QuestLog,
    db: &QuestDb,
    pins: &QuestPins,
    ui: &LedgerUi,
    now_unix: u64,
) -> LedgerView {
    let count = |state| log.iter().filter(|e| e.state == state).count() as u32;
    let ready = log.iter().filter(|e| is_ready(e)).count();
    let tabs = [
        (LedgerTab::Active, "Active", QuestState::Active),
        (LedgerTab::Shelved, "Shelved", QuestState::Inactive),
        (LedgerTab::Completed, "Completed", QuestState::Complete),
    ]
    .map(|(tab, label, state)| TabView {
        tab,
        label,
        count: count(state),
        selected: tab == ui.tab,
    })
    .to_vec();

    let in_tab: Vec<&QuestEntry> = log
        .iter()
        .filter(|entry| tab_of(entry.state) == ui.tab)
        .collect();
    let query = ui.query.trim().to_lowercase();
    let row = |entry: &QuestEntry| row_view(entry, db, pins, ui, now_unix);
    let matching = in_tab
        .iter()
        .copied()
        .filter(|entry| matches_query(entry, db, &query));
    let groups = match ui.tab {
        LedgerTab::Active => {
            let (ready_rows, rest): (Vec<_>, Vec<_>) = matching.partition(|e| is_ready(e));
            [("READY TO TURN IN", ready_rows), ("IN PROGRESS", rest)]
                .into_iter()
                .filter(|(_, rows)| !rows.is_empty())
                .map(|(label, rows)| GroupView {
                    label: Some(label),
                    rows: rows.into_iter().map(row).collect(),
                })
                .collect()
        }
        _ => vec![GroupView {
            label: None,
            rows: matching.map(row).collect(),
        }],
    };

    let detail = ui
        .selected
        .and_then(|id| in_tab.iter().copied().find(|e| e.quest_id == id))
        .map(|entry| detail_view(entry, db, now_unix));

    LedgerView {
        headline: format!(
            "{} active · {ready} ready to turn in",
            count(QuestState::Active)
        ),
        tabs,
        groups,
        detail,
        pin_note: format!("{} of {MAX_PINS} tracked on screen", pins.ids().len()),
    }
}

pub fn tracker_view(log: &QuestLog, db: &QuestDb, pins: &QuestPins, now_unix: u64) -> TrackerView {
    let quests: Vec<TrackerQuest> = pins
        .ids()
        .iter()
        .filter_map(|id| log.get(*id))
        .filter(|entry| entry.state == QuestState::Active)
        .map(|entry| TrackerQuest {
            id: entry.quest_id,
            title: title(entry, db),
            steps: steps(entry, db),
            ready_text: ready_text(entry),
            timer: timer(entry, now_unix),
        })
        .collect();
    TrackerView { quests }
}

/// `"Defeat Zerom"`, or a description of the filter when the mob has no name.
pub fn objective_label(objective: &QuestObjective) -> String {
    let target = match objective.mob_name.is_empty() {
        false => objective.mob_name.clone(),
        true => monster_kind(&objective.filter),
    };
    let mut label = format!("Defeat {target}");
    match (objective.filter.min_level, objective.filter.max_level) {
        (0, 0) => {}
        (lo, 0) => label += &format!(" Lv {lo}+"),
        (lo, hi) => label += &format!(" Lv {lo}-{hi}"),
    }
    if !objective.filter.map.is_empty() {
        label += &format!(" in {}", objective.filter.map);
    }
    label
}

/// Time left until `deadline`, in the two largest units that fit.
pub fn countdown(deadline: u64, now_unix: u64) -> String {
    let left = deadline.saturating_sub(now_unix);
    match left {
        0 => "Expired".into(),
        DAY.. => format!("{} d {} h", left / DAY, left % DAY / HOUR),
        HOUR.. => format!("{} h {} m", left / HOUR, left % HOUR / MINUTE),
        _ => match left.div_ceil(MINUTE) {
            60 => "1 h 0 m".into(),
            minutes => format!("{minutes} m"),
        },
    }
}

fn tab_of(state: QuestState) -> LedgerTab {
    match state {
        QuestState::Active => LedgerTab::Active,
        QuestState::Inactive => LedgerTab::Shelved,
        QuestState::Complete => LedgerTab::Completed,
    }
}

fn title(entry: &QuestEntry, db: &QuestDb) -> String {
    match db.text(entry.quest_id) {
        Some(text) if !text.title.is_empty() => text.title.clone(),
        _ if !entry.title.is_empty() => entry.title.clone(),
        _ => format!("Quest #{}", entry.quest_id),
    }
}

fn matches_query(entry: &QuestEntry, db: &QuestDb, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let giver = entry.giver.iter().flat_map(|g| [&g.name, &g.map]);
    std::iter::once(title(entry, db))
        .chain(giver.cloned())
        .any(|text| text.to_lowercase().contains(query))
}

fn done_steps(steps: &[StepView]) -> usize {
    steps.iter().filter(|s| s.done).count()
}

fn row_view(
    entry: &QuestEntry,
    db: &QuestDb,
    pins: &QuestPins,
    ui: &LedgerUi,
    now_unix: u64,
) -> RowView {
    let on_active_tab = entry.state == QuestState::Active;
    let ready = is_ready(entry);
    let chip = match (on_active_tab, ready) {
        (false, _) => None,
        (true, true) => Some(Chip::Ready),
        (true, false) => entry.deadline.map(|d| Chip::Ends(chip_text(d, now_unix))),
    };
    let step_list = steps(entry, db);
    let giver = entry.giver.as_ref().map(giver_name);
    let progress = format!("{} of {} done", done_steps(&step_list), step_list.len());
    let sub_line = match (entry.state, giver, step_list.is_empty()) {
        (QuestState::Complete, giver, _) => giver.unwrap_or_default(),
        (_, Some(giver), false) => format!("{giver} · {progress}"),
        (_, Some(giver), true) => giver,
        (_, None, false) => progress,
        (_, None, true) => String::new(),
    };
    RowView {
        id: entry.quest_id,
        title: title(entry, db),
        sub_line,
        chip,
        bar_pct: bar_pct(entry),
        pinned: pins.contains(entry.quest_id),
        selected: ui.selected == Some(entry.quest_id),
        ready,
    }
}

/// The text of a deadline: `"Ends {countdown}"`, or `"Expired"`.
pub(super) fn chip_text(deadline: u64, now_unix: u64) -> String {
    ends(countdown(deadline, now_unix))
}

/// `"Ends {left}"` for a [`countdown`], which already reads `"Expired"` at zero.
pub(super) fn ends(left: String) -> String {
    match left.as_str() {
        "Expired" => left,
        _ => format!("Ends {left}"),
    }
}

fn giver_name(giver: &QuestGiver) -> String {
    match giver.name.is_empty() {
        true => giver.map.clone(),
        false => giver.name.clone(),
    }
}

fn giver_line(giver: &QuestGiver) -> String {
    let place = format!("{} ({}, {})", giver.map, giver.x, giver.y);
    match giver.name.is_empty() {
        true => place,
        false => format!("{} · {place}", giver.name),
    }
}

fn bar_pct(entry: &QuestEntry) -> u8 {
    if entry.state == QuestState::Complete {
        return 100;
    }
    let needed: u64 = entry.objectives.iter().map(|o| u64::from(o.needed)).sum();
    let current: u64 = entry
        .objectives
        .iter()
        .map(|o| u64::from(o.current.min(o.needed)))
        .sum();
    match needed {
        0 => 0,
        _ => (current * 100 / needed) as u8,
    }
}

fn step_pct(objective: &QuestObjective) -> u8 {
    match objective.needed {
        0 => 100,
        needed => (u64::from(objective.current.min(needed)) * 100 / u64::from(needed)) as u8,
    }
}

/// The objectives as steps. A quest with none borrows its catalog summary, or the
/// first description line, as its one step.
fn steps(entry: &QuestEntry, db: &QuestDb) -> Vec<StepView> {
    if !entry.objectives.is_empty() {
        return entry
            .objectives
            .iter()
            .map(|o| StepView {
                label: objective_label(o),
                count_text: format!("{}/{}", o.current, o.needed),
                bar_pct: step_pct(o),
                done: o.current >= o.needed,
            })
            .collect();
    }
    let Some(text) = db.text(entry.quest_id) else {
        return Vec::new();
    };
    let label = match text.summary.is_empty() {
        false => text.summary.clone(),
        true => text.description.first().cloned().unwrap_or_default(),
    };
    match label.is_empty() {
        true => Vec::new(),
        false => {
            let done = entry.state == QuestState::Complete;
            vec![StepView {
                label,
                count_text: String::new(),
                bar_pct: if done { 100 } else { 0 },
                done,
            }]
        }
    }
}

fn ready_text(entry: &QuestEntry) -> Option<String> {
    if !is_ready(entry) {
        return None;
    }
    Some(match &entry.giver {
        Some(giver) => format!("Ready · Return to {}", giver_name(giver)),
        None => "Ready to turn in".into(),
    })
}

fn timer(entry: &QuestEntry, now_unix: u64) -> Option<String> {
    match entry.state {
        QuestState::Active => entry.deadline.map(|d| countdown(d, now_unix)),
        _ => None,
    }
}

fn detail_view(entry: &QuestEntry, db: &QuestDb, now_unix: u64) -> DetailView {
    DetailView {
        id: entry.quest_id,
        title: title(entry, db),
        giver_line: entry.giver.as_ref().map(giver_line),
        timer: timer(entry, now_unix),
        story: db
            .text(entry.quest_id)
            .map(|text| text.description.clone())
            .unwrap_or_default(),
        steps: steps(entry, db),
        can_navigate: entry.giver.is_some(),
        shelve: match entry.state {
            QuestState::Active => Some(ShelveAction::Shelve),
            QuestState::Inactive => Some(ShelveAction::Resume),
            QuestState::Complete => None,
        },
        ready_text: ready_text(entry),
    }
}

/// `"Large Fire Demi-Human monsters"`, or `"monsters"` for an empty filter.
fn monster_kind(filter: &HuntFilter) -> String {
    let words = [
        display_word(&filter.size),
        display_word(&filter.element),
        display_word(&filter.race),
    ];
    let mut kind: Vec<String> = words.into_iter().flatten().collect();
    kind.push("monsters".into());
    kind.join(" ")
}

/// The quest_db spelling made readable. `all` and empty mean "no restriction".
fn display_word(raw: &str) -> Option<String> {
    let lower = raw.to_lowercase();
    match lower.as_str() {
        "" | "all" => None,
        "demihuman" => Some("Demi-Human".into()),
        _ => {
            let mut chars = lower.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
        }
    }
}
