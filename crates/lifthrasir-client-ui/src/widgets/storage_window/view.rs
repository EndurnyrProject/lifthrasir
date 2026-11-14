//! Pure view-models of the Storage window: what each pane lists and in which order,
//! which stacks the bulk buttons move, and the selection bar's read-outs. Nothing
//! here touches the ECS, so every rule is unit-testable on plain data.

use bevy::prelude::*;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_client::domain::inventory::item::item_category;
use lifthrasir_client::domain::inventory::{Inventory, Item, ItemCategory};
use lifthrasir_client::domain::storage::Storage;
use lifthrasir_ui::theme::ro_dark;
use net_contract::dto::{StorageItem, StorageKind};

use super::{Side, StorageCategory, StorageSelection, StorageSort, StorageUi, Transfer};
use crate::screens::shared::thousands;
use crate::widgets::inventory_window::scene::CellView;
use crate::widgets::inventory_window::{self, LooseItem, cell_view};

/// Everything outside [`StorageUi`] that the window draws from.
pub(crate) struct Sources<'a> {
    pub inventory: &'a Inventory,
    pub storage: &'a Storage,
    pub item_db: Option<&'a ItemDb>,
    /// The local player's guild, named in the guild vault's caption.
    pub guild: Option<&'a str>,
    /// Carried weight and its limit.
    pub weight: (u32, u32),
    pub zeny: u32,
}

/// One pane: its caption and its filled slots in display order.
pub(crate) struct PaneView {
    pub side: Side,
    pub title: &'static str,
    pub count: String,
    pub hint: (String, Color),
    pub cells: Vec<(StorageSelection, CellView)>,
    pub scroll: Vec2,
}

/// The facts the selection bar's second line is written from. It rides on that
/// line's entity, so a new amount rewrites the text without rebuilding the bar.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub(crate) struct MetaLine {
    pub place: &'static str,
    pub stack: u32,
    /// Per-unit weight. Only vault items carry one.
    pub unit_weight: Option<u32>,
    pub carried: (u32, u32),
}

impl MetaLine {
    /// The line and its colour for a move of `qty`: where the item is and, for a
    /// withdrawal, the carried weight after it or a warning when it will not fit.
    pub fn line(&self, qty: u32) -> (String, Color) {
        let place = format!("{} \u{b7} {}", self.place, thousands(self.stack.into()));
        let Some(unit) = self.unit_weight else {
            return (place, ro_dark::MUTED);
        };
        let (carried, limit) = self.carried;
        let after = u64::from(carried) + u64::from(unit) * u64::from(qty);
        if unit > 0 && limit > 0 && after > u64::from(limit) {
            let room = limit.saturating_sub(carried) / unit;
            return (
                format!(
                    "Too heavy \u{b7} You can carry {} more",
                    thousands(room.into())
                ),
                ro_dark::STATUS_FULL,
            );
        }
        let each = if self.stack > 1 { " each" } else { "" };
        (
            format!(
                "{place} \u{b7} Weight {}{each} \u{b7} Bag after: {} / {}",
                thousands(unit.into()),
                thousands(after),
                thousands(limit.into())
            ),
            ro_dark::MUTED,
        )
    }
}

/// The selected item as the bar under the panes shows it.
pub(crate) struct SelectionView {
    pub source: StorageSelection,
    pub cell: CellView,
    pub name: String,
    pub stack: u32,
    pub meta: MetaLine,
    /// Shown in place of the amount controls when the slot holds one item.
    pub note: &'static str,
}

/// The rebuilt part of the window.
pub(crate) struct BodyView {
    pub bag: PaneView,
    pub vault: PaneView,
    pub selection: Option<SelectionView>,
    pub qty: u32,
    /// A stack was dropped across and the bar is asking how many to move.
    pub asking: bool,
    /// Transfers are in progress, so every control that starts one is off.
    pub busy: bool,
    pub loot: usize,
    pub top_up: usize,
    pub sort: StorageSort,
    /// Used vault slots and their limit.
    pub capacity: (u32, u32),
    pub weight: (u32, u32),
    pub zeny: u32,
    /// The last finished action, which Undo reverses.
    pub last: Option<String>,
}

fn vault_slot(item: &StorageItem) -> LooseItem<'_> {
    LooseItem {
        nameid: item.nameid,
        identified: item.identified,
        refine: item.refine,
        cards: &item.cards,
        amount: item.amount,
    }
}

/// Whether an item passes the category segment and the search text. `query` is
/// already trimmed and lowercased by [`StorageUi::set_query`].
pub(crate) fn matches_filter(
    selected: StorageCategory,
    query: &str,
    category: ItemCategory,
    name: &str,
) -> bool {
    let in_category = match selected {
        StorageCategory::All => true,
        StorageCategory::Use => category == ItemCategory::Use,
        StorageCategory::Equip => category == ItemCategory::Equip,
        StorageCategory::Etc => category == ItemCategory::Etc,
    };
    in_category && name.to_lowercase().contains(query)
}

/// Usables first, then equipment, then everything else.
fn type_rank(category: ItemCategory) -> u8 {
    match category {
        ItemCategory::Use => 0,
        ItemCategory::Equip => 1,
        ItemCategory::Etc => 2,
    }
}

/// The Bag pane's items: everything not worn that passes the filter, by type then
/// name.
pub(crate) fn bag_items<'a>(
    inventory: &'a Inventory,
    item_db: Option<&ItemDb>,
    ui: &StorageUi,
) -> Vec<&'a Item> {
    let mut items: Vec<(String, &Item)> = inventory
        .iter()
        .filter(|item| !item.is_equipped())
        .map(|item| (inventory_window::item_name(item, item_db), item))
        .filter(|(name, item)| matches_filter(ui.category, ui.query(), item.category(), name))
        .collect();
    items.sort_by(|(a_name, a), (b_name, b)| {
        type_rank(a.category())
            .cmp(&type_rank(b.category()))
            .then_with(|| a_name.cmp(b_name))
    });
    items.into_iter().map(|(_, item)| item).collect()
}

/// The Vault pane's items: everything that passes the filter, in the chosen order.
/// Ties keep vault-slot order.
pub(crate) fn vault_items<'a>(
    storage: &'a Storage,
    item_db: Option<&ItemDb>,
    ui: &StorageUi,
) -> Vec<&'a StorageItem> {
    let mut items: Vec<(String, &StorageItem)> = storage
        .iter()
        .map(|item| (vault_slot(item).name(item_db), item))
        .filter(|(name, item)| {
            matches_filter(ui.category, ui.query(), item_category(item.type_), name)
        })
        .collect();
    let rank = |item: &StorageItem| type_rank(item_category(item.type_));
    items.sort_by(|(a_name, a), (b_name, b)| match ui.sort {
        StorageSort::Type => rank(a).cmp(&rank(b)).then_with(|| a_name.cmp(b_name)),
        StorageSort::Name => a_name.cmp(b_name),
        StorageSort::Amount => b.amount.cmp(&a.amount),
        StorageSort::Recent => ui.recency(b.index).cmp(&ui.recency(a.index)),
    });
    items.into_iter().map(|(_, item)| item).collect()
}

/// Whether items of this `IT_*` type share a slot. Armor, weapons, pet eggs, pet
/// armor and shadow gear each take their own.
fn stacks(item_type: u32) -> bool {
    !matches!(item_type, 4 | 5 | 7 | 8 | 12)
}

/// What "Deposit loot" moves: every whole Etc stack that is not a favorite.
///
/// NOTE: the client cannot tell refine materials (Elunium, Oridecon) from other
/// loot, so they count. Skip them once item data carries that.
pub(crate) fn loot(inventory: &Inventory) -> Vec<Transfer> {
    inventory
        .iter()
        .filter(|item| !item.is_equipped() && !item.favorite)
        .filter(|item| item.category() == ItemCategory::Etc)
        .map(whole_stack)
        .collect()
}

/// What "Top up stacks" moves: every whole bag stack, favorites aside, of an item
/// the vault already holds.
pub(crate) fn top_up(inventory: &Inventory, storage: &Storage) -> Vec<Transfer> {
    inventory
        .iter()
        .filter(|item| !item.is_equipped() && !item.favorite)
        .filter(|item| stacks(item.item_type.into()))
        .filter(|item| storage.iter().any(|slot| slot.nameid == item.item_id))
        .map(whole_stack)
        .collect()
}

fn whole_stack(item: &Item) -> Transfer {
    Transfer {
        source: StorageSelection::Bag(item.index),
        amount: item.amount.into(),
    }
}

/// How many the `source` slot holds right now, or `None` once it is gone or worn.
pub(crate) fn available(
    source: StorageSelection,
    inventory: &Inventory,
    storage: &Storage,
) -> Option<u32> {
    match source {
        StorageSelection::Bag(index) => inventory
            .get(index)
            .filter(|item| !item.is_equipped())
            .map(|item| item.amount.into()),
        StorageSelection::Vault(index) => storage.get(index).map(|item| item.amount),
    }
}

/// The name of the item in `source` as the window writes it, as in `+7 Blade [3]`.
pub(crate) fn name(
    source: StorageSelection,
    inventory: &Inventory,
    storage: &Storage,
    item_db: Option<&ItemDb>,
) -> Option<String> {
    match source {
        StorageSelection::Bag(index) => inventory
            .get(index)
            .map(|item| inventory_window::item_name(item, item_db)),
        StorageSelection::Vault(index) => storage
            .get(index)
            .map(|item| vault_slot(item).name(item_db)),
    }
}

/// What the commit button says for `qty` of an item selected in `source`.
pub(crate) fn action_label(source: StorageSelection, qty: u32) -> String {
    let verb = match source.side() {
        Side::Bag => "Deposit",
        Side::Vault => "Withdraw",
    };
    format!("{verb} {}", thousands(qty.into()))
}

/// How far along the amount slider `qty` of a `stack` sits, in percent.
pub(crate) fn slider_percent(qty: u32, stack: u32) -> f32 {
    if stack <= 1 {
        return 100.0;
    }
    (qty.clamp(1, stack) - 1) as f32 / (stack - 1) as f32 * 100.0
}

/// The amount a slider fraction (0 at the left end, 1 at the right) picks from a
/// `stack`.
pub(crate) fn slider_amount(fraction: f32, stack: u32) -> u32 {
    let span = stack.saturating_sub(1) as f32;
    1 + (fraction.clamp(0.0, 1.0) * span).round() as u32
}

fn selection_view(sources: &Sources, selected: Option<StorageSelection>) -> Option<SelectionView> {
    let source = selected?;
    let carried = sources.weight;
    match source {
        StorageSelection::Bag(index) => {
            let item = sources
                .inventory
                .get(index)
                .filter(|item| !item.is_equipped())?;
            let stack = u32::from(item.amount);
            let note = match (item.favorite, item.category()) {
                (true, _) => "Favorite \u{b7} Kept out of bulk deposits",
                (false, ItemCategory::Equip) => "Equipment never stacks",
                (false, _) => "Single item",
            };
            Some(SelectionView {
                source,
                cell: cell_view(item, sources.item_db, None),
                name: inventory_window::item_name(item, sources.item_db),
                stack,
                meta: MetaLine {
                    place: "In your bag",
                    stack,
                    unit_weight: None,
                    carried,
                },
                note,
            })
        }
        StorageSelection::Vault(index) => {
            let item = sources.storage.get(index)?;
            let slot = vault_slot(item);
            let place = match sources.storage.kind() {
                Some(StorageKind::Guild) => "In the guild vault",
                _ => "In your vault",
            };
            let note = match item_category(item.type_) {
                ItemCategory::Equip => "Equipment never stacks",
                _ => "Single item",
            };
            Some(SelectionView {
                source,
                cell: slot.cell(sources.item_db, false),
                name: slot.name(sources.item_db),
                stack: item.amount,
                meta: MetaLine {
                    place,
                    stack: item.amount,
                    unit_weight: Some(item.weight),
                    carried,
                },
                note,
            })
        }
    }
}

fn bag_pane(sources: &Sources, ui: &StorageUi) -> PaneView {
    let selected = match ui.selected {
        Some(StorageSelection::Bag(index)) => Some(index),
        _ => None,
    };
    let (weight, limit) = sources.weight;
    PaneView {
        side: Side::Bag,
        title: "BAG",
        count: format!(
            "{} / {}",
            sources.inventory.len(),
            inventory_window::CAPACITY
        ),
        hint: (
            format!(
                "Weight {} / {}",
                thousands(weight.into()),
                thousands(limit.into())
            ),
            ro_dark::MUTED,
        ),
        cells: bag_items(sources.inventory, sources.item_db, ui)
            .into_iter()
            .map(|item| {
                (
                    StorageSelection::Bag(item.index),
                    cell_view(item, sources.item_db, selected),
                )
            })
            .collect(),
        scroll: Vec2::ZERO,
    }
}

fn vault_pane(sources: &Sources, ui: &StorageUi) -> PaneView {
    let storage = sources.storage;
    let (title, hint) = match storage.kind() {
        Some(StorageKind::Guild) => (
            "GUILD VAULT",
            (
                format!(
                    "Shared with {} \u{b7} Withdrawals are logged",
                    sources.guild.unwrap_or("your guild")
                ),
                ro_dark::STATUS_BUSY,
            ),
        ),
        _ => (
            "VAULT",
            (
                "Personal \u{b7} Shared by all your heroes".to_string(),
                ro_dark::MUTED,
            ),
        ),
    };
    PaneView {
        side: Side::Vault,
        title,
        count: format!("{} / {}", storage.len(), storage.capacity()),
        hint,
        cells: vault_items(storage, sources.item_db, ui)
            .into_iter()
            .map(|item| {
                let source = StorageSelection::Vault(item.index);
                (
                    source,
                    vault_slot(item).cell(sources.item_db, ui.selected == Some(source)),
                )
            })
            .collect(),
        scroll: Vec2::ZERO,
    }
}

/// Everything the rebuilt body draws. Both panes start scrolled to the top; the
/// caller carries their offsets over.
pub(crate) fn body_view(sources: &Sources, ui: &StorageUi) -> BodyView {
    let storage = sources.storage;
    BodyView {
        bag: bag_pane(sources, ui),
        vault: vault_pane(sources, ui),
        selection: selection_view(sources, ui.selected),
        qty: ui.qty,
        asking: ui.asking,
        busy: ui.busy(),
        loot: loot(sources.inventory).len(),
        top_up: top_up(sources.inventory, storage).len(),
        sort: ui.sort,
        capacity: (storage.len() as u32, storage.capacity()),
        weight: sources.weight,
        zeny: sources.zeny,
        last: ui.last_action().map(str::to_string),
    }
}
