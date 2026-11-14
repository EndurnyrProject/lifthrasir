//! Storage window: the Kafra vault beside the bag, following the `Storage` artboard
//! of the Lifthrasir Widgets design.
//!
//! The frame ([`scene::build`]) holds the title bar, the search field and two
//! [`StorageHost`] regions that [`rebuild`] respawns: the category segments and the
//! body (both panes around the move arrows, the selection bar, the read-outs). The
//! amount lives in its field between rebuilds; [`sync_amount`] keeps it,
//! [`StorageUi::qty`] and the read-outs that name it in step.
//!
//! The server opens and picks the vault (a Kafra script), so the window only shows
//! while [`Storage`] is open and never switches vaults itself.
//!
//! Every move is server-authoritative and the protocol carries one stack per
//! request, answered by a [`StorageResult`] with no request id. So each action,
//! single or bulk, is a [`Run`]: a queue [`drive_transfers`] sends one transfer at a
//! time, waiting for the answer before the next. The slot each transfer landed in is
//! read from the server's deltas, which is what lets Undo send the moves back.

use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use bevy::ecs::system::SystemParam;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::Activate;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::catalogs::random_option::RandomOptionDb;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::guild::GuildState;
use lifthrasir_client::domain::inventory::Inventory;
use lifthrasir_client::domain::storage::Storage;
use lifthrasir_ui::theme::ro_dark;
use net_contract::commands::{CloseStorage, DepositStorageItem, WithdrawStorageItem};
use net_contract::dto::StorageKind;
use net_contract::events::{ItemAdded, StorageItemAdded, StorageRejection, StorageResult};

use crate::screens::shared::{
    light_field_focus, light_search_focus, set_field_text, shown_if, thousands, typed_number,
};
use crate::widgets::info_modal::{InfoContent, ShowInfoModal, view as info_view};

mod feedback;
pub mod scene;
#[cfg(test)]
mod tests;
pub(crate) mod view;

const DOUBLE_CLICK: Duration = Duration::from_millis(300);
/// How long a toast stays up.
const TOAST: Duration = Duration::from_millis(2600);

#[derive(Component, Default, Clone)]
pub struct StorageWindowRoot;

/// The draggable title bar.
#[derive(Component, Default, Clone)]
pub struct StorageWindowTitlebar;

/// The search text, which filters both panes.
#[derive(Component, Default, Clone)]
pub struct StorageSearchField;

/// The typed amount of the selected stack.
#[derive(Component, Default, Clone)]
pub struct StorageAmountField;

type StorageFieldFilter = Or<(With<StorageSearchField>, With<StorageAmountField>)>;

/// A region of the frame that [`rebuild`] respawns.
#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum StorageHost {
    #[default]
    Filters,
    Body,
}

/// The scrolling grid of one pane. Its offset survives a rebuild, and it is where a
/// slot dragged from the other pane is dropped.
#[derive(Component, Default, Clone, Copy)]
pub struct StoragePane(pub Side);

/// A filled slot and the container slot it shows.
#[derive(Component, Clone, Copy)]
pub struct StorageCell(pub StorageSelection);

/// The commit button's label, which names the amount it moves.
#[derive(Component, Default, Clone)]
pub struct AmountLabel;

/// The amount slider's track; a press or a drag on it sets the amount.
#[derive(Component, Default, Clone)]
pub struct AmountRail;

#[derive(Component, Default, Clone)]
pub struct AmountFill;

#[derive(Component, Default, Clone)]
pub struct AmountKnob;

/// The pill that names the last outcome for a moment.
#[derive(Component, Default, Clone)]
pub struct StorageToast;

/// Which container a slot belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Side {
    #[default]
    Bag,
    Vault,
}

/// A slot of the bag (inventory index) or of the open vault (storage index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageSelection {
    Bag(u16),
    Vault(u32),
}

impl StorageSelection {
    pub fn side(self) -> Side {
        match self {
            Self::Bag(_) => Side::Bag,
            Self::Vault(_) => Side::Vault,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum StorageCategory {
    #[default]
    All,
    Use,
    Equip,
    Etc,
}

impl StorageCategory {
    pub const ALL: [Self; 4] = [Self::All, Self::Use, Self::Equip, Self::Etc];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Use => "Use",
            Self::Equip => "Equip",
            Self::Etc => "Etc",
        }
    }
}

/// The order of the Vault pane. The Bag pane is always by type.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum StorageSort {
    #[default]
    Type,
    Name,
    Amount,
    /// Slots that received items this session first, newest on top.
    Recent,
}

impl StorageSort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Type => "Type",
            Self::Name => "Name",
            Self::Amount => "Amount",
            Self::Recent => "Recent",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Type => Self::Name,
            Self::Name => Self::Amount,
            Self::Amount => Self::Recent,
            Self::Recent => Self::Type,
        }
    }
}

/// The amount chips beside the slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    One,
    Ten,
    Half,
    All,
}

impl Preset {
    pub const ALL: [Self; 4] = [Self::One, Self::Ten, Self::Half, Self::All];

    pub fn label(self) -> &'static str {
        match self {
            Self::One => "1",
            Self::Ten => "10",
            Self::Half => "Half",
            Self::All => "All",
        }
    }

    /// The amount the chip picks from a `stack`.
    pub fn of(self, stack: u32) -> u32 {
        let amount = match self {
            Self::One => 1,
            Self::Ten => 10,
            Self::Half => stack / 2,
            Self::All => stack,
        };
        amount.clamp(1, stack.max(1))
    }
}

/// What a control of the window does. Art buttons report it through
/// [`on_storage_button`], flat controls through [`on_storage_click`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageAction {
    Category(StorageCategory),
    CycleSort,
    /// One more (`true`) or one less of the selected stack.
    Step(bool),
    Amount(Preset),
    /// Move the chosen amount of the selected item across.
    Commit,
    DepositLoot,
    TopUp,
    Undo,
}

/// One request: `amount` of the item in `source`, moved to the other side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transfer {
    pub source: StorageSelection,
    pub amount: u32,
}

/// A transfer the server accepted and the slot it landed in, when the server's
/// deltas told. Undo needs the landing slot, so a move without one stays put.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Done {
    pub moved: Transfer,
    pub landed: Option<StorageSelection>,
}

/// What a [`Run`] is, which names it once it ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RunKind {
    /// One stack of the named item.
    Move(String),
    Loot,
    TopUp,
    /// The reverse of the action with this text.
    Undo(String),
}

/// An action in progress: the transfers still to send, the one awaiting its answer
/// and the ones already accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Run {
    kind: RunKind,
    queue: VecDeque<Transfer>,
    in_flight: Option<Transfer>,
    /// Where the transfer in flight landed, read from the server's deltas.
    landed: Option<StorageSelection>,
    done: Vec<Done>,
    /// The last rejection, reported when the run ends.
    failed: Option<StorageRejection>,
}

impl Run {
    /// The line that names the finished run. `first` is its first accepted transfer.
    fn text(&self, first: Transfer) -> String {
        let stacks = match self.done.len() {
            1 => "1 stack".to_string(),
            count => format!("{count} stacks"),
        };
        match &self.kind {
            RunKind::Move(name) => {
                let verb = match first.source.side() {
                    Side::Bag => "Deposited",
                    Side::Vault => "Withdrew",
                };
                format!("{verb} {} {name}", thousands(first.amount.into()))
            }
            RunKind::Loot => format!("Deposited loot \u{b7} {stacks}"),
            RunKind::TopUp => format!("Topped up {stacks} in the vault"),
            RunKind::Undo(text) => format!("Undid: {text}"),
        }
    }
}

/// A finished action as Undo keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Action {
    pub text: String,
    pub moves: Vec<Done>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub text: String,
    /// A rejection, drawn in the warning colours.
    pub bad: bool,
    /// The [`Time::elapsed`] at which it goes away.
    pub until: Duration,
}

/// The window's whole UI state. It is reset whenever the vault opens, closes or
/// changes, so nothing here outlives the storage session its slot indices are from.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct StorageUi {
    pub category: StorageCategory,
    query: String,
    pub sort: StorageSort,
    pub selected: Option<StorageSelection>,
    /// How many of the selected stack the next commit moves.
    pub qty: u32,
    /// A stack was dropped across: the selection bar asks how many before moving.
    pub asking: bool,
    pub(crate) run: Option<Run>,
    /// Finished actions, newest last. Undo pops one.
    pub(crate) history: Vec<Action>,
    pub toast: Option<Toast>,
    /// The order vault slots last received items in, for [`StorageSort::Recent`].
    recent: HashMap<u32, u32>,
    last_click: Option<(StorageSelection, Duration)>,
}

impl Default for StorageUi {
    fn default() -> Self {
        Self {
            category: StorageCategory::default(),
            query: String::new(),
            sort: StorageSort::default(),
            selected: None,
            qty: 1,
            asking: false,
            run: None,
            history: Vec::new(),
            toast: None,
            recent: HashMap::new(),
            last_click: None,
        }
    }
}

/// Whether a rejection means the rest of a queue would fail too. The others are
/// about the one item, so a bulk action skips it and carries on.
fn stops_queue(rejection: StorageRejection) -> bool {
    !matches!(
        rejection,
        StorageRejection::NotStorable
            | StorageRejection::ItemEquipped
            | StorageRejection::InvalidAmount
            | StorageRejection::Rental
    )
}

impl StorageUi {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.trim().to_lowercase();
    }

    /// Whether an action is in progress. Nothing else may start until it ends.
    pub(crate) fn busy(&self) -> bool {
        self.run.is_some()
    }

    /// The transfer waiting for the server's answer.
    pub(crate) fn in_flight(&self) -> Option<Transfer> {
        self.run.as_ref()?.in_flight
    }

    /// The text of the action Undo would reverse.
    pub(crate) fn last_action(&self) -> Option<&str> {
        self.history.last().map(|action| action.text.as_str())
    }

    /// Rank of a vault slot under [`StorageSort::Recent`]: higher is newer, zero is
    /// untouched this session.
    pub(crate) fn recency(&self, index: u32) -> u32 {
        self.recent.get(&index).copied().unwrap_or(0)
    }

    /// Queues `moves` as one action. Ignored while another is in progress.
    pub(crate) fn begin(&mut self, kind: RunKind, moves: Vec<Transfer>) {
        if self.busy() || moves.is_empty() {
            return;
        }
        self.asking = false;
        self.run = Some(Run {
            kind,
            queue: moves.into(),
            in_flight: None,
            landed: None,
            done: Vec::new(),
            failed: None,
        });
    }

    /// Sends the last action back: its transfers reversed, newest first, each from
    /// the slot it landed in.
    ///
    /// NOTE: the landing slots are as the server reported them then. If the bag
    /// changed since (an item used up), the server rejects that step and the undo
    /// stops there.
    pub(crate) fn undo(&mut self) {
        if self.busy() {
            return;
        }
        let Some(action) = self.history.pop() else {
            return;
        };
        let moves = action
            .moves
            .iter()
            .rev()
            .filter_map(|done| {
                Some(Transfer {
                    source: done.landed?,
                    amount: done.moved.amount,
                })
            })
            .collect();
        self.begin(RunKind::Undo(action.text), moves);
    }

    /// A vault slot received items: it becomes the newest, and it is where a deposit
    /// in flight landed.
    fn note_stored(&mut self, index: u32) {
        let newest = self.recent.values().max().copied().unwrap_or(0) + 1;
        self.recent.insert(index, newest);
        self.land(StorageSelection::Vault(index));
    }

    /// Records where the transfer in flight landed, when `slot` is on its far side.
    fn land(&mut self, slot: StorageSelection) {
        let Some(run) = &mut self.run else {
            return;
        };
        if run
            .in_flight
            .is_some_and(|moved| moved.source.side() != slot.side())
        {
            run.landed = Some(slot);
        }
    }

    /// Records the server's answer to the transfer in flight. `false` when nothing
    /// was in flight.
    pub(crate) fn settle(&mut self, outcome: Result<(), StorageRejection>) -> bool {
        let Some(run) = &mut self.run else {
            return false;
        };
        let Some(moved) = run.in_flight.take() else {
            return false;
        };
        let landed = run.landed.take();
        match outcome {
            Ok(()) => run.done.push(Done { moved, landed }),
            Err(rejection) => {
                run.failed = Some(rejection);
                if stops_queue(rejection) {
                    run.queue.clear();
                }
            }
        }
        true
    }

    /// Ends the run: names it in a toast, with the rejection that cut it short if
    /// any, and keeps what it moved for Undo. An undo is not itself undoable.
    pub(crate) fn finish(&mut self, now: Duration) {
        let Some(run) = self.run.take() else {
            return;
        };
        let until = now + TOAST;
        let failure = run.failed.map(rejection_message);
        let Some(first) = run.done.first() else {
            self.toast = failure.map(|text| Toast {
                text,
                bad: true,
                until,
            });
            return;
        };
        let text = run.text(first.moved);
        self.toast = Some(match failure {
            Some(why) => Toast {
                text: format!("{text} \u{b7} then: {why}"),
                bad: true,
                until,
            },
            None => Toast {
                text: text.clone(),
                bad: false,
                until,
            },
        });
        if !matches!(run.kind, RunKind::Undo(_)) {
            self.history.push(Action {
                text,
                moves: run.done,
            });
        }
    }
}

pub(crate) fn rejection_message(rejection: StorageRejection) -> String {
    match rejection {
        StorageRejection::Full => "Storage is full.".to_string(),
        StorageRejection::InventoryFull => "Your inventory is full.".to_string(),
        StorageRejection::Overweight => "You are carrying too much weight.".to_string(),
        StorageRejection::NotStorable => "This item cannot be stored.".to_string(),
        StorageRejection::ItemEquipped => "Equipped items cannot be stored.".to_string(),
        StorageRejection::InvalidAmount => "Enter a valid amount.".to_string(),
        StorageRejection::NotOpen => "Storage is not open.".to_string(),
        StorageRejection::BasicSkillRequired => "Basic Skill level 6 is required.".to_string(),
        StorageRejection::NoGuild => "You are not in a guild.".to_string(),
        StorageRejection::GuildNoSkill => "Your guild needs the Guild Storage skill.".to_string(),
        StorageRejection::GuildNoPermission => {
            "You do not have permission to use guild storage.".to_string()
        }
        StorageRejection::GuildInUse => "Guild storage is already in use.".to_string(),
        StorageRejection::OtherStorageOpen => "Close your other storage window first.".to_string(),
        StorageRejection::Rental => "Rental items cannot be stored here.".to_string(),
        StorageRejection::NoGuildStorage => "Guild storage is unavailable.".to_string(),
        StorageRejection::Stale => "Your storage session has expired. Reopen storage.".to_string(),
        StorageRejection::Unknown(code) => format!("Storage request failed (code {code})."),
    }
}

/// The bag, the vault and the item catalogues, as the window reads them.
#[derive(SystemParam)]
pub(crate) struct StorageItems<'w> {
    inventory: Res<'w, Inventory>,
    storage: Res<'w, Storage>,
    item_db: Option<Res<'w, ItemDb>>,
    option_db: Option<Res<'w, RandomOptionDb>>,
}

/// The window's state beside the containers it acts on: everything a click needs.
#[derive(SystemParam)]
pub(crate) struct StorageDesk<'w> {
    ui: ResMut<'w, StorageUi>,
    items: StorageItems<'w>,
}

impl StorageDesk<'_> {
    fn available(&self, source: StorageSelection) -> Option<u32> {
        view::available(source, &self.items.inventory, &self.items.storage)
    }

    fn selected_stack(&self) -> u32 {
        self.ui
            .selected
            .and_then(|source| self.available(source))
            .unwrap_or(1)
    }

    fn set_qty(&mut self, qty: u32) {
        self.ui.qty = qty.clamp(1, self.selected_stack().max(1));
    }

    /// Selects a slot with its whole stack as the amount. `true` when this click
    /// completes a double-click on it.
    fn select(&mut self, source: StorageSelection, now: Duration) -> bool {
        let Some(stack) = self.available(source) else {
            return false;
        };
        let double = self
            .ui
            .last_click
            .is_some_and(|(last, at)| last == source && now.saturating_sub(at) <= DOUBLE_CLICK);
        self.ui.last_click = (!double).then_some((source, now));
        self.ui.selected = Some(source);
        self.ui.qty = stack;
        self.ui.asking = false;
        double
    }

    /// Starts moving `amount` of the item in `source` across.
    fn transfer(&mut self, source: StorageSelection, amount: u32) {
        let items = &self.items;
        let Some(name) = view::name(
            source,
            &items.inventory,
            &items.storage,
            items.item_db.as_deref(),
        ) else {
            return;
        };
        self.ui
            .begin(RunKind::Move(name), vec![Transfer { source, amount }]);
    }

    /// A slot dragged onto the other pane: one item moves at once, a stack is
    /// selected whole and the bar asks how many.
    fn drop_across(&mut self, source: StorageSelection) {
        let Some(stack) = self.available(source) else {
            return;
        };
        if stack == 1 {
            self.transfer(source, 1);
            return;
        }
        self.ui.selected = Some(source);
        self.ui.qty = stack;
        self.ui.asking = true;
    }

    fn act(&mut self, action: StorageAction) {
        match action {
            StorageAction::Category(category) => self.ui.category = category,
            StorageAction::CycleSort => self.ui.sort = self.ui.sort.next(),
            StorageAction::Step(more) => {
                let qty = self.ui.qty;
                self.set_qty(if more {
                    qty.saturating_add(1)
                } else {
                    qty.saturating_sub(1)
                });
            }
            StorageAction::Amount(preset) => {
                let stack = self.selected_stack();
                self.set_qty(preset.of(stack));
            }
            StorageAction::Commit => {
                if let Some(source) = self.ui.selected {
                    let qty = self.ui.qty;
                    self.transfer(source, qty);
                }
            }
            StorageAction::DepositLoot => {
                let moves = view::loot(&self.items.inventory);
                self.ui.begin(RunKind::Loot, moves);
            }
            StorageAction::TopUp => {
                let moves = view::top_up(&self.items.inventory, &self.items.storage);
                self.ui.begin(RunKind::TopUp, moves);
            }
            StorageAction::Undo => self.ui.undo(),
        }
    }

    /// The info modal's content for the item in `source`.
    fn inspect(&self, source: StorageSelection) -> Option<InfoContent> {
        let items = &self.items;
        let Some(item_db) = items.item_db.as_deref() else {
            warn!("storage: ItemDb not loaded yet, ignoring inspect");
            return None;
        };
        let option_db = items.option_db.as_deref();
        let view = match source {
            StorageSelection::Bag(index) => items
                .inventory
                .get(index)
                .map(|item| info_view::inventory_item_view(item, item_db, option_db)),
            StorageSelection::Vault(index) => items
                .storage
                .get(index)
                .map(|item| info_view::storage_item_view(item, item_db, option_db)),
        };
        view.map(|view| InfoContent::Item { view, action: None })
    }
}

/// An art button of the window was pressed.
pub(crate) fn on_storage_button(
    activate: On<Activate>,
    actions: Query<&StorageAction>,
    mut desk: StorageDesk,
) {
    if let Ok(action) = actions.get(activate.entity) {
        desk.act(*action);
    }
}

/// A flat control of the window (a category segment, the sort box, an amount chip)
/// was clicked.
pub(crate) fn on_storage_click(
    click: On<PointerClick>,
    actions: Query<&StorageAction>,
    mut desk: StorageDesk,
) {
    if let Ok(action) = actions.get(click.entity) {
        desk.act(*action);
    }
}

/// A click selects the slot with its whole stack as the amount, and a double-click
/// moves that stack across. A secondary click opens the info modal instead.
pub(crate) fn on_cell_click(
    click: On<PointerClick>,
    cells: Query<&StorageCell>,
    time: Res<Time>,
    mut desk: StorageDesk,
    mut info_writer: MessageWriter<ShowInfoModal>,
) {
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    if click.button == PointerButton::Secondary {
        if let Some(content) = desk.inspect(cell.0) {
            info_writer.write(ShowInfoModal { content });
        }
        return;
    }
    if desk.select(cell.0, time.elapsed()) {
        let stack = desk.selected_stack();
        desk.transfer(cell.0, stack);
    }
}

/// Dragging a slot lights the pane it can be dropped on.
pub(crate) fn on_cell_drag_start(
    drag: On<PointerDragStart>,
    cells: Query<&StorageCell>,
    mut panes: Query<(&StoragePane, &mut Outline)>,
) {
    let Ok(cell) = cells.get(drag.entity) else {
        return;
    };
    for (pane, mut outline) in &mut panes {
        if pane.0 != cell.0.side() {
            outline.color = ro_dark::FROST_GLOW;
        }
    }
}

pub(crate) fn on_cell_drag_end(
    _: On<PointerDragEnd>,
    mut panes: Query<&mut Outline, With<StoragePane>>,
) {
    for mut outline in &mut panes {
        outline.color = Color::NONE;
    }
}

/// A slot dropped on a pane. The drop bubbles up from whatever lies under the
/// pointer, so the pane is found from the grid itself or from one of its slots.
pub(crate) fn on_pane_drop(
    mut drop: On<PointerDragDrop>,
    panes: Query<&StoragePane>,
    cells: Query<&StorageCell>,
    mut desk: StorageDesk,
) {
    drop.propagate(false);
    let target = panes
        .get(drop.entity)
        .map(|pane| pane.0)
        .or_else(|_| cells.get(drop.entity).map(|cell| cell.0.side()));
    let (Ok(target), Ok(dragged)) = (target, cells.get(drop.dropped)) else {
        return;
    };
    if dragged.0.side() != target {
        desk.drop_across(dragged.0);
    }
}

/// Sets the amount from where the pointer is along the slider.
fn set_amount_from_cursor(
    rail: Entity,
    rails: &Query<&RelativeCursorPosition, With<AmountRail>>,
    desk: &mut StorageDesk,
) {
    // `normalized` is centred on the node: -0.5 at its left edge, 0.5 at its right.
    let Some(at) = rails.get(rail).ok().and_then(|cursor| cursor.normalized) else {
        return;
    };
    let qty = view::slider_amount(at.x + 0.5, desk.selected_stack());
    if desk.ui.qty != qty {
        desk.ui.qty = qty;
    }
}

pub(crate) fn on_slider_press(
    press: On<PointerPress>,
    rails: Query<&RelativeCursorPosition, With<AmountRail>>,
    mut desk: StorageDesk,
) {
    set_amount_from_cursor(press.entity, &rails, &mut desk);
}

pub(crate) fn on_slider_drag(
    drag: On<PointerDrag>,
    rails: Query<&RelativeCursorPosition, With<AmountRail>>,
    mut desk: StorageDesk,
) {
    set_amount_from_cursor(drag.entity, &rails, &mut desk);
}

/// The close button asks the server to end the session. The window hides once the
/// domain has closed [`Storage`].
pub(crate) fn on_storage_close(
    _: On<Activate>,
    storage: Res<Storage>,
    mut close: MessageWriter<CloseStorage>,
) {
    if let Some(kind) = storage.kind() {
        close.write(CloseStorage { kind });
    }
}

/// Shows the window while [`Storage`] is open. When the vault opens, closes or
/// changes, the UI state starts over and the fields are emptied and unfocused.
fn sync_window(
    storage: Res<Storage>,
    mut previous: Local<Option<StorageKind>>,
    mut roots: Query<&mut Visibility, With<StorageWindowRoot>>,
    mut searches: Query<&mut EditableText, With<StorageSearchField>>,
    fields: Query<(), StorageFieldFilter>,
    mut input_focus: ResMut<InputFocus>,
    mut ui: ResMut<StorageUi>,
) {
    let Ok(mut visibility) = roots.single_mut() else {
        return;
    };
    let shown = shown_if(storage.is_open());
    if *visibility != shown {
        *visibility = shown;
    }
    if *previous == storage.kind() {
        return;
    }
    *previous = storage.kind();
    for mut search in &mut searches {
        search.clear();
    }
    if input_focus
        .get()
        .is_some_and(|entity| fields.contains(entity))
    {
        input_focus.clear();
    }
    *ui = StorageUi::default();
}

fn sync_search(fields: Query<&EditableText, With<StorageSearchField>>, mut ui: ResMut<StorageUi>) {
    let Ok(field) = fields.single() else {
        return;
    };
    let typed = field.value().to_string();
    if typed.trim().to_lowercase() != ui.query() {
        ui.set_query(&typed);
    }
}

/// The server's storage answers and deltas, and the two transfer commands.
#[derive(SystemParam)]
pub(crate) struct TransferWire<'w, 's> {
    results: MessageReader<'w, 's, StorageResult>,
    stored: MessageReader<'w, 's, StorageItemAdded>,
    carried: MessageReader<'w, 's, ItemAdded>,
    deposit: MessageWriter<'w, DepositStorageItem>,
    withdraw: MessageWriter<'w, WithdrawStorageItem>,
}

/// Runs the action in progress: reads where the transfer in flight landed and the
/// server's answer to it, then sends the next one or ends the run. A queued slot
/// that has vanished is skipped, and an amount is capped to what the slot holds.
fn drive_transfers(mut wire: TransferWire, mut desk: StorageDesk, time: Res<Time>) {
    let StorageDesk { ui, items } = &mut desk;
    let Some(kind) = items.storage.kind() else {
        let stale = wire.results.read().count();
        wire.stored.clear();
        wire.carried.clear();
        if stale > 0 {
            warn!("ignoring Storage result while Storage is closed");
        }
        return;
    };
    for added in wire.stored.read() {
        if added.kind == kind {
            ui.note_stored(added.item.index);
        }
    }
    for added in wire.carried.read() {
        if ui.in_flight().is_some() {
            ui.land(StorageSelection::Bag(added.index as u16));
        }
    }
    for result in wire.results.read() {
        if let Err(StorageRejection::Unknown(code)) = result.outcome {
            warn!(code, "unknown Storage rejection code");
        }
        if ui.in_flight().is_none() {
            warn!("ignoring Storage result without an in-flight request");
            continue;
        }
        ui.settle(result.outcome);
    }
    if ui.run.is_none() || ui.in_flight().is_some() {
        return;
    }
    let Some(run) = &mut ui.run else {
        return;
    };
    while let Some(next) = run.queue.pop_front() {
        let Some(stack) = view::available(next.source, &items.inventory, &items.storage) else {
            continue;
        };
        let amount = next.amount.min(stack);
        match next.source {
            StorageSelection::Bag(index) => {
                wire.deposit.write(DepositStorageItem {
                    kind,
                    inventory_index: index.into(),
                    amount,
                });
            }
            StorageSelection::Vault(index) => {
                wire.withdraw.write(WithdrawStorageItem {
                    kind,
                    storage_index: index,
                    amount,
                });
            }
        }
        run.in_flight = Some(Transfer { amount, ..next });
        run.landed = None;
        return;
    }
    ui.finish(time.elapsed());
}

/// The local player's status and guild.
#[derive(SystemParam)]
pub(crate) struct StorageOwner<'w, 's> {
    statuses: Query<'w, 's, &'static CharacterStatus, With<LocalPlayer>>,
    guild: Res<'w, GuildState>,
}

/// What [`rebuild`] last drew that change detection cannot tell it about.
/// [`StorageUi::qty`] and the toast are left out: they are written in place.
#[derive(Default, PartialEq)]
pub(crate) struct Shown {
    category: StorageCategory,
    query: String,
    sort: StorageSort,
    selected: Option<StorageSelection>,
    asking: bool,
    busy: bool,
    history: usize,
    weight: (u32, u32),
    zeny: u32,
}

/// Respawns the [`StorageHost`] regions when the bag, the vault or anything in
/// [`Shown`] changes, and once when they first spawn. `CharacterStatus` changes on
/// every HP tick, so its shown weight and zeny are compared instead of trusting
/// change detection. Typing an amount never respawns its field, and both panes keep
/// their scroll offsets.
///
/// An open vault without the local player's `CharacterStatus` skips the pass loudly
/// rather than drawing a fabricated footer.
fn rebuild(
    mut commands: Commands,
    items: StorageItems,
    mut ui: ResMut<StorageUi>,
    owner: StorageOwner,
    hosts: Query<(Entity, Ref<StorageHost>, Option<&Children>)>,
    panes: Query<(&StoragePane, &ScrollPosition)>,
    mut shown: Local<Shown>,
) {
    if !items.storage.is_open() {
        return;
    }
    let Some(status) = owner.statuses.iter().next() else {
        warn!("storage window rebuild skipped: no LocalPlayer/CharacterStatus");
        return;
    };
    let gone = ui
        .selected
        .is_some_and(|source| view::available(source, &items.inventory, &items.storage).is_none());
    if gone {
        ui.selected = None;
        ui.asking = false;
    }
    let now = Shown {
        category: ui.category,
        query: ui.query().to_string(),
        sort: ui.sort,
        selected: ui.selected,
        asking: ui.asking,
        busy: ui.busy(),
        history: ui.history.len(),
        weight: (status.weight, status.max_weight),
        zeny: status.zeny,
    };
    let dirty = items.inventory.is_changed()
        || items.storage.is_changed()
        || owner.guild.is_changed()
        || hosts.iter().any(|(_, host, _)| host.is_added())
        || *shown != now;
    if !dirty {
        return;
    }
    let sources = view::Sources {
        inventory: &items.inventory,
        storage: &items.storage,
        item_db: items.item_db.as_deref(),
        guild: owner.guild.info().map(|info| info.name.as_str()),
        weight: now.weight,
        zeny: now.zeny,
    };
    let scroll_of = |side| {
        panes
            .iter()
            .find(|(pane, _)| pane.0 == side)
            .map(|(_, position)| position.0)
            .unwrap_or_default()
    };
    for (host, region, children) in &hosts {
        for child in children.into_iter().flatten() {
            commands.entity(*child).despawn();
        }
        match *region {
            StorageHost::Filters => {
                commands
                    .spawn_scene(scene::filters(ui.category))
                    .insert(ChildOf(host));
            }
            StorageHost::Body => {
                let mut body = view::body_view(&sources, &ui);
                body.bag.scroll = scroll_of(Side::Bag);
                body.vault.scroll = scroll_of(Side::Vault);
                commands
                    .spawn_scene(scene::body(body))
                    .insert(ChildOf(host));
            }
        }
    }
    *shown = now;
}

/// The parts of the selection bar that name the amount.
#[derive(SystemParam)]
pub(crate) struct AmountParts<'w, 's> {
    fields: Query<'w, 's, &'static mut EditableText, With<StorageAmountField>>,
    labels: Query<'w, 's, &'static mut Text, (With<AmountLabel>, Without<view::MetaLine>)>,
    metas: Query<
        'w,
        's,
        (
            &'static view::MetaLine,
            &'static mut Text,
            &'static mut TextColor,
        ),
        Without<AmountLabel>,
    >,
    fills: Query<'w, 's, &'static mut Node, (With<AmountFill>, Without<AmountKnob>)>,
    knobs: Query<'w, 's, &'static mut Node, (With<AmountKnob>, Without<AmountFill>)>,
}

/// Keeps the typed amount and [`StorageUi::qty`] in step, capped to the selected
/// stack, and rewrites what names the amount: the commit label, the meta line and
/// the slider. A stepper, a chip, the slider or a new selection rewrites the field;
/// a typed number becomes the amount. An emptied field keeps the last amount, so the
/// player can clear it and type another.
fn sync_amount(
    mut ui: ResMut<StorageUi>,
    items: StorageItems,
    mut parts: AmountParts,
    mut synced: Local<u32>,
) {
    let Some(source) = ui.selected else {
        return;
    };
    let stack = view::available(source, &items.inventory, &items.storage)
        .unwrap_or(1)
        .max(1);
    let capped = ui.qty.clamp(1, stack);
    if ui.qty != capped {
        ui.qty = capped;
    }
    if let Ok(mut field) = parts.fields.single_mut() {
        let typed = typed_number(&field);
        match typed.map(|typed| typed.clamp(1, stack)) {
            Some(wanted) if ui.qty == *synced && wanted != ui.qty => ui.qty = wanted,
            _ => {}
        }
        if typed != Some(ui.qty) && (typed.is_some() || ui.qty != *synced) {
            set_field_text(&mut field, &ui.qty.to_string());
        }
    }
    if ui.qty == *synced {
        return;
    }
    *synced = ui.qty;
    let label = view::action_label(source, ui.qty);
    for mut text in &mut parts.labels {
        text.0.clone_from(&label);
    }
    for (meta, mut text, mut color) in &mut parts.metas {
        let (line, tone) = meta.line(ui.qty);
        text.0 = line;
        color.0 = tone;
    }
    let at = Val::Percent(view::slider_percent(ui.qty, stack));
    for mut fill in &mut parts.fills {
        fill.width = at;
    }
    for mut knob in &mut parts.knobs {
        knob.left = at;
    }
}

/// Takes the toast down once its time is up and writes the current one, or hides
/// the pill.
fn show_toast(
    time: Res<Time>,
    mut ui: ResMut<StorageUi>,
    mut pills: Query<(&mut Visibility, &Children), With<StorageToast>>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    let expired = ui
        .toast
        .as_ref()
        .is_some_and(|toast| toast.until <= time.elapsed());
    if expired {
        ui.toast = None;
    }
    if !ui.is_changed() {
        return;
    }
    for (mut visibility, children) in &mut pills {
        let shown = shown_if(ui.toast.is_some());
        if *visibility != shown {
            *visibility = shown;
        }
        let Some(toast) = &ui.toast else {
            continue;
        };
        let ink = if toast.bad {
            ro_dark::STATUS_FULL
        } else {
            ro_dark::BODY
        };
        for child in children.iter() {
            let Ok((mut text, mut color)) = texts.get_mut(child) else {
                continue;
            };
            if text.0 != toast.text {
                text.0.clone_from(&toast.text);
            }
            color.0 = ink;
        }
    }
}

fn reset_storage_ui(mut ui: ResMut<StorageUi>) {
    *ui = StorageUi::default();
}

pub struct StorageWindowPlugin;

impl Plugin for StorageWindowPlugin {
    fn build(&self, app: &mut App) {
        use lifthrasir_client::domain::inventory::systems::apply_item_deltas;
        use lifthrasir_client::domain::storage::systems::{
            apply_storage_close, apply_storage_item_deltas, apply_storage_opened,
        };

        app.init_resource::<StorageUi>()
            .add_systems(
                Update,
                (
                    sync_window,
                    // Before `drive_transfers` settles the transfer in flight, so an
                    // answer to it is not also reported as unprompted.
                    feedback::ingest_unprompted_errors,
                    drive_transfers,
                    sync_search,
                    rebuild,
                    sync_amount,
                    show_toast,
                    light_search_focus::<StorageSearchField>,
                    light_field_focus::<StorageAmountField>,
                )
                    .chain()
                    .after(apply_item_deltas)
                    .after(apply_storage_opened)
                    .after(apply_storage_item_deltas)
                    .after(apply_storage_close)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(OnExit(GameState::InGame), reset_storage_ui);
    }
}
