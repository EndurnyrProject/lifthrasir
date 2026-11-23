//! The quest ledger window and tracker plate, after the `Quest` artboard of the
//! Lifthrasir Widgets design.
//!
//! The server owns the quest log: this module projects it through [`view`] and turns
//! the window's controls into [`QuestCommand`] and [`NavigationRequested`]. Each
//! [`Host`] is a region [`rebuild`] refills when its view changes. Deadlines tick in
//! place once a second, so a running countdown never rebuilds a region.
//!
//! Every system runs only while the [`QuestDb`] catalog is loaded.

pub mod view;

mod scene;

#[cfg(test)]
mod tests;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bevy::ecs::system::SystemParam;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::time::common_conditions::on_timer;
use bevy::ui_widgets::Activate;
use leafwing_input_manager::prelude::ActionState;
use lifthrasir_assets::catalogs::QuestDb;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::input::{PlayerAction, ui_unfocused};
use lifthrasir_client::domain::quest::{MAX_PINS, PinRefused, QuestLog, QuestPins};
use net_contract::commands::{NavigationRequested, QuestCommand};
use net_contract::dto::{NavigationTarget, QuestState};

use crate::screens::shared::light_search_focus;
use view::{Chip, DetailView, GroupView, LedgerView, TabView, ledger_view};

/// Seconds the in-window toast stays up.
const TOAST_SECONDS: f32 = 2.6;

pub struct QuestLedgerPlugin;
impl Plugin for QuestLedgerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LedgerUi>()
            .add_systems(
                Update,
                (
                    toggle_key.run_if(ui_unfocused),
                    update_window,
                    sync_search,
                    expire_toast,
                    rebuild,
                    tick_timers.run_if(on_timer(Duration::from_secs(1))),
                    light_search_focus::<QuestSearch>.run_if(resource_exists::<InputFocus>),
                )
                    .chain()
                    .run_if(in_state(GameState::InGame))
                    .run_if(resource_exists::<QuestDb>),
            )
            .add_systems(OnExit(GameState::InGame), reset);
    }
}

/// Which list of quests the ledger shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LedgerTab {
    #[default]
    Active,
    Shelved,
    Completed,
}

/// What the player has done in the ledger window.
#[derive(Resource, Default, Debug)]
pub struct LedgerUi {
    pub open: bool,
    pub tab: LedgerTab,
    pub selected: Option<u32>,
    pub query: String,
    /// A message and the seconds it has left on screen.
    pub toast: Option<(String, f32)>,
}

#[derive(Component, Default, Clone)]
struct QuestLedgerWindow;
/// The draggable title bar.
#[derive(Component, Default, Clone)]
struct QuestLedgerTitlebar;
/// The search box, mirrored into [`LedgerUi::query`].
#[derive(Component, Default, Clone)]
struct QuestSearch;

/// A deadline of the quest with this id, rewritten in place by [`tick_timers`]. It
/// sits on a node whose first child is the text.
#[derive(Component, Default, Clone, Copy)]
struct TimerText(u32);

/// A region [`rebuild`] refills when its view changes.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
enum Host {
    /// Beside the title: how many quests are active and ready.
    #[default]
    Lead,
    Tabs,
    List,
    /// Under the list: how many quests are tracked on screen.
    Note,
    Detail,
    Footer,
    Toast,
}

const HOSTS: usize = 7;

/// What a control of the window does.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
enum Act {
    #[default]
    Close,
    Tab(LedgerTab),
    Select(u32),
    Pin(u32),
    Navigate,
    Shelve,
    Resume,
}

/// Spawns the hidden window under the HUD root.
pub fn spawn_window(commands: &mut Commands, parent: Entity) {
    commands
        .spawn_scene(scene::window())
        .insert(ChildOf(parent));
}

fn toggle_key(
    actions: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut ui: ResMut<LedgerUi>,
) {
    if actions
        .single()
        .is_ok_and(|keys| keys.just_pressed(&PlayerAction::QuestLedger))
    {
        ui.open = !ui.open;
    }
}

fn reset(mut ui: ResMut<LedgerUi>) {
    *ui = LedgerUi::default();
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Everything a control reads or changes.
#[derive(SystemParam)]
struct Desk<'w> {
    ui: ResMut<'w, LedgerUi>,
    log: Res<'w, QuestLog>,
    pins: ResMut<'w, QuestPins>,
    quests: MessageWriter<'w, QuestCommand>,
    routes: MessageWriter<'w, NavigationRequested>,
}

impl Desk<'_> {
    fn run(&mut self, act: Act) {
        match act {
            Act::Close => self.ui.open = false,
            Act::Tab(tab) => self.ui.tab = tab,
            Act::Select(id) => self.ui.selected = Some(id),
            Act::Pin(id) => self.pin(id),
            Act::Navigate => self.navigate(),
            Act::Shelve => self.set_active(QuestState::Active, false),
            Act::Resume => self.set_active(QuestState::Inactive, true),
        }
    }

    fn pin(&mut self, id: u32) {
        if self.pins.toggle(id, &self.log) == Err(PinRefused::Full) {
            let text = format!("You can track up to {MAX_PINS} quests on screen.");
            self.ui.toast = Some((text, TOAST_SECONDS));
        }
    }

    fn navigate(&mut self) {
        let Some(giver) = self
            .ui
            .selected
            .and_then(|id| self.log.get(id))
            .and_then(|entry| entry.giver.as_ref())
        else {
            return;
        };
        self.routes.write(NavigationRequested {
            target: NavigationTarget::Coord {
                map: giver.map.clone(),
                x: giver.x,
                y: giver.y,
            },
            flag: 0,
            hide_window: false,
        });
    }

    /// Asks the server to move the selected quest out of `from`. The log changes only
    /// when the server answers.
    fn set_active(&mut self, from: QuestState, active: bool) {
        let Some(entry) = self.ui.selected.and_then(|id| self.log.get(id)) else {
            return;
        };
        if entry.state != from {
            return;
        }
        self.quests.write(QuestCommand::SetActive {
            quest_id: entry.quest_id,
            active,
        });
    }
}

/// Runs the [`Act`] of an activated button. The headless button also activates on a
/// right click, which is ignored.
fn on_button(
    activate: On<Activate>,
    acts: Query<&Act>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut desk: Desk,
) {
    if mouse.is_some_and(|mouse| mouse.just_released(MouseButton::Right)) {
        return;
    }
    if let Ok(act) = acts.get(activate.entity) {
        desk.run(*act);
    }
}

/// Runs the [`Act`] of a clicked tab or row.
fn on_click(click: On<PointerClick>, acts: Query<&Act>, mut desk: Desk) {
    if click.button != PointerButton::Primary {
        return;
    }
    if let Ok(act) = acts.get(click.entity) {
        desk.run(*act);
    }
}

/// Shows or hides the window.
fn update_window(ui: Res<LedgerUi>, mut windows: Query<&mut Node, With<QuestLedgerWindow>>) {
    let display = if ui.open {
        Display::Flex
    } else {
        Display::None
    };
    for mut window in &mut windows {
        if window.display != display {
            window.display = display;
        }
    }
}

/// Mirrors the search box into [`LedgerUi::query`].
fn sync_search(fields: Query<&EditableText, With<QuestSearch>>, mut ui: ResMut<LedgerUi>) {
    let Ok(text) = fields.single() else {
        return;
    };
    let value = text.value().to_string();
    if ui.query != value {
        ui.query = value;
    }
}

fn expire_toast(time: Res<Time>, mut ui: ResMut<LedgerUi>) {
    let Some((_, left)) = ui.toast.as_ref() else {
        return;
    };
    let left = left - time.delta_secs();
    if left <= 0.0 {
        ui.toast = None;
    } else if let Some((_, seconds)) = ui.toast.as_mut() {
        *seconds = left;
    }
}

/// Rewrites every shown deadline from the log.
fn tick_timers(
    log: Res<QuestLog>,
    timers: Query<(&TimerText, &Children)>,
    mut texts: Query<&mut Text>,
) {
    let now = unix_now();
    for (timer, children) in &timers {
        let Some(deadline) = log.get(timer.0).and_then(|entry| entry.deadline) else {
            continue;
        };
        let Some(mut text) = children.first().and_then(|c| texts.get_mut(*c).ok()) else {
            continue;
        };
        let label = view::chip_text(deadline, now);
        if text.0 != label {
            text.0 = label;
        }
    }
}

/// What a [`Host`] shows.
#[derive(Clone, PartialEq)]
enum Content {
    Lead(String),
    Tabs(Vec<TabView>),
    List(Vec<GroupView>, LedgerTab),
    Note(String),
    Detail(DetailView),
    /// The parchment while no quest is picked.
    Pick,
    /// Whether the giver can be routed to, the shelve label and the ready line.
    Footer(bool, Option<&'static str>, Option<String>),
    Toast(String),
}

impl Content {
    fn spawn(self, commands: &mut Commands) -> Entity {
        match self {
            Content::Lead(text) => commands.spawn_scene(scene::lead(text)).id(),
            Content::Tabs(tabs) => commands.spawn_scene(scene::tabs(tabs)).id(),
            Content::List(groups, tab) => commands.spawn_scene(scene::list(groups, tab)).id(),
            Content::Note(text) => commands.spawn_scene(scene::note(text)).id(),
            Content::Detail(view) => commands.spawn_scene(scene::detail(view)).id(),
            Content::Pick => commands.spawn_scene(scene::pick()).id(),
            Content::Footer(navigate, shelve, ready) => commands
                .spawn_scene(scene::footer(navigate, shelve, ready))
                .id(),
            Content::Toast(text) => commands.spawn_scene(scene::toast(text)).id(),
        }
    }

    /// The content with its countdowns blanked, which [`tick_timers`] keeps current, so
    /// a ticking deadline does not count as a change.
    fn settled(mut self) -> Self {
        match &mut self {
            Content::List(groups, _) => {
                let chips = groups.iter_mut().flat_map(|g| g.rows.iter_mut());
                for row in chips {
                    if let Some(Chip::Ends(text)) = &mut row.chip {
                        text.clear();
                    }
                }
            }
            Content::Detail(view) => {
                if let Some(text) = &mut view.timer {
                    text.clear();
                }
            }
            _ => {}
        }
        self
    }
}

/// The world as the window reads it.
#[derive(SystemParam)]
struct Seen<'w> {
    ui: Res<'w, LedgerUi>,
    log: Res<'w, QuestLog>,
    pins: Res<'w, QuestPins>,
    db: Res<'w, QuestDb>,
}

impl Seen<'_> {
    fn view(&self) -> Option<LedgerView> {
        self.ui
            .open
            .then(|| ledger_view(&self.log, &self.db, &self.pins, &self.ui, unix_now()))
    }

    /// What `host` shows now. Every host is empty while the window is closed.
    fn content(&self, host: Host, view: Option<&LedgerView>) -> Option<Content> {
        let view = view?;
        Some(match host {
            Host::Lead => Content::Lead(view.headline.clone()),
            Host::Tabs => Content::Tabs(view.tabs.clone()),
            Host::List => Content::List(view.groups.clone(), self.ui.tab),
            Host::Note => Content::Note(view.pin_note.clone()),
            Host::Detail => view.detail.clone().map_or(Content::Pick, Content::Detail),
            Host::Footer => {
                let d = view.detail.as_ref()?;
                Content::Footer(d.can_navigate, d.shelve_label, d.ready_text.clone())
            }
            Host::Toast => Content::Toast(self.ui.toast.as_ref()?.0.clone()),
        })
    }
}

/// Refills every [`Host`] whose view changed.
fn rebuild(
    mut commands: Commands,
    seen: Seen,
    hosts: Query<(Entity, Ref<Host>, Option<&Children>)>,
    mut shown: Local<[Option<Content>; HOSTS]>,
) {
    let view = seen.view();
    for (host, kind, children) in &hosts {
        let content = seen.content(*kind, view.as_ref());
        let settled = content.clone().map(Content::settled);
        let slot = &mut shown[*kind as usize];
        if *slot == settled && !kind.is_added() {
            continue;
        }
        for child in children.into_iter().flatten() {
            commands.entity(*child).despawn();
        }
        if let Some(content) = content {
            let spawned = content.spawn(&mut commands);
            commands.entity(spawned).insert(ChildOf(host));
        }
        *slot = settled;
    }
}
