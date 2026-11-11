//! The Character window's equipment panel: the Equipment / Costume / Shadow Gear tabs,
//! two columns of captioned slots either side of the preview, and the bar that names
//! the selected item and unequips it.
//!
//! [`EquipUi`] is the only UI state. [`equipment_column`] builds the static frame with
//! its [`EquipMount`]s, and [`rebuild_equipment`] respawns their content whenever the
//! inventory or [`EquipUi`] changes. A click selects a slot, a double-click unequips
//! it and a secondary click opens the shared info modal.

use std::time::Duration;

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::catalogs::random_option::RandomOptionDb;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::equipment::UnequipItemRequested;
use lifthrasir_client::domain::equipment::location::{
    EQP_AMMO, EQP_ARMOR, EQP_COSTUME_GARMENT, EQP_COSTUME_HEAD_LOW, EQP_COSTUME_HEAD_MID,
    EQP_COSTUME_HEAD_TOP, EQP_GARMENT, EQP_HEAD_LOW, EQP_HEAD_MID, EQP_HEAD_TOP,
    EQP_LEFT_ACCESSORY, EQP_LEFT_HAND, EQP_RIGHT_ACCESSORY, EQP_RIGHT_HAND, EQP_SHADOW_ARMOR,
    EQP_SHADOW_LEFT_ACCESSORY, EQP_SHADOW_RIGHT_ACCESSORY, EQP_SHADOW_SHIELD, EQP_SHADOW_SHOES,
    EQP_SHADOW_WEAPON, EQP_SHOES,
};
use lifthrasir_client::domain::inventory::{Inventory, Item};
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    BUTTON_SHADOW, ButtonArt, HoverColors, inset, tab, tab_text,
};

use crate::screens::shared::{flush_button, overline};
use crate::widgets::character_window::{attributes, preview};
use crate::widgets::info_modal::{InfoContent, ItemAction, ShowInfoModal, view};
use crate::widgets::inventory_window::scene::{CellView, Ring, slot_frame, slot_marks};
use crate::widgets::inventory_window::{cell_view, item_name};

const SLOT: f32 = 50.0;
const ICON: f32 = 36.0;
const SELECTION_SLOT: f32 = 44.0;
const SELECTION_ICON: f32 = 32.0;
const SLOT_COLUMN: f32 = 178.0;
const CAPTION: f32 = 120.0;
const DOUBLE_CLICK: Duration = Duration::from_millis(300);

/// Which paperdoll slot a well represents.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CharEquipSlotKind {
    #[default]
    HeadUpper,
    HeadMid,
    HeadLower,
    Body,
    Garment,
    RightHand,
    LeftHand,
    AccessoryRight,
    AccessoryLeft,
    Footgear,
    Ammo,
}

impl CharEquipSlotKind {
    pub fn wear_bit(self) -> u32 {
        match self {
            CharEquipSlotKind::HeadUpper => EQP_HEAD_TOP,
            CharEquipSlotKind::HeadMid => EQP_HEAD_MID,
            CharEquipSlotKind::HeadLower => EQP_HEAD_LOW,
            CharEquipSlotKind::Body => EQP_ARMOR,
            CharEquipSlotKind::Garment => EQP_GARMENT,
            CharEquipSlotKind::RightHand => EQP_RIGHT_HAND,
            CharEquipSlotKind::LeftHand => EQP_LEFT_HAND,
            CharEquipSlotKind::AccessoryRight => EQP_RIGHT_ACCESSORY,
            CharEquipSlotKind::AccessoryLeft => EQP_LEFT_ACCESSORY,
            CharEquipSlotKind::Footgear => EQP_SHOES,
            CharEquipSlotKind::Ammo => EQP_AMMO,
        }
    }

    /// The slot's name in the Equipment set.
    pub fn slot_label(self) -> &'static str {
        match self {
            CharEquipSlotKind::HeadUpper => "Upper Headgear",
            CharEquipSlotKind::HeadMid => "Mid Headgear",
            CharEquipSlotKind::HeadLower => "Lower Headgear",
            CharEquipSlotKind::Body => "Body",
            CharEquipSlotKind::Garment => "Garment",
            CharEquipSlotKind::RightHand => "Right Hand",
            CharEquipSlotKind::LeftHand => "Left Hand",
            CharEquipSlotKind::AccessoryRight => "Accessory",
            CharEquipSlotKind::AccessoryLeft => "Accessory",
            CharEquipSlotKind::Footgear => "Footgear",
            CharEquipSlotKind::Ammo => "Ammunition",
        }
    }
}

/// The gear set a tab shows. Every set lays its items out on the same eleven slots
/// and leaves the ones it has no use for disabled.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum EquipSet {
    #[default]
    Equipment,
    Costume,
    Shadow,
}

impl EquipSet {
    pub const ALL: [Self; 3] = [Self::Equipment, Self::Costume, Self::Shadow];

    pub fn label(self) -> &'static str {
        match self {
            Self::Equipment => "Equipment",
            Self::Costume => "Costume",
            Self::Shadow => "Shadow Gear",
        }
    }

    /// The wear bit and name of `kind` in this set, or `None` where the set has no
    /// such slot.
    pub fn slot(self, kind: CharEquipSlotKind) -> Option<(u32, &'static str)> {
        use CharEquipSlotKind as Kind;
        match (self, kind) {
            (Self::Equipment, kind) => Some((kind.wear_bit(), kind.slot_label())),
            (Self::Costume, Kind::HeadUpper) => Some((EQP_COSTUME_HEAD_TOP, "Costume Upper")),
            (Self::Costume, Kind::HeadMid) => Some((EQP_COSTUME_HEAD_MID, "Costume Mid")),
            (Self::Costume, Kind::HeadLower) => Some((EQP_COSTUME_HEAD_LOW, "Costume Lower")),
            (Self::Costume, Kind::Garment) => Some((EQP_COSTUME_GARMENT, "Costume Garment")),
            (Self::Shadow, Kind::Body) => Some((EQP_SHADOW_ARMOR, "Shadow Armor")),
            (Self::Shadow, Kind::RightHand) => Some((EQP_SHADOW_WEAPON, "Shadow Weapon")),
            (Self::Shadow, Kind::LeftHand) => Some((EQP_SHADOW_SHIELD, "Shadow Shield")),
            (Self::Shadow, Kind::AccessoryRight) => {
                Some((EQP_SHADOW_RIGHT_ACCESSORY, "Shadow Earring"))
            }
            (Self::Shadow, Kind::AccessoryLeft) => {
                Some((EQP_SHADOW_LEFT_ACCESSORY, "Shadow Pendant"))
            }
            (Self::Shadow, Kind::Footgear) => Some((EQP_SHADOW_SHOES, "Shadow Shoes")),
            _ => None,
        }
    }
}

/// The panel's whole UI state.
#[derive(Resource, Default)]
pub struct EquipUi {
    pub set: EquipSet,
    pub selected: Option<CharEquipSlotKind>,
}

/// A region [`rebuild_equipment`] fills.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum EquipMount {
    #[default]
    Tabs,
    Left,
    Right,
    Selection,
}

#[derive(Component, Clone, Copy, Default)]
pub struct EquipTabButton(pub EquipSet);

/// A filled slot, carrying the inventory index of the item it shows.
#[derive(Component, Clone, Copy, Default)]
pub struct EquipSlot {
    pub kind: CharEquipSlotKind,
    pub index: u16,
}

/// A slot's place in a column: which kind it is and the empty-state glyph icon name.
#[derive(Clone, Copy)]
pub struct SlotSpec {
    pub kind: CharEquipSlotKind,
    pub glyph: &'static str,
}

const fn slot(kind: CharEquipSlotKind, glyph: &'static str) -> SlotSpec {
    SlotSpec { kind, glyph }
}

/// Left column, top-to-bottom: the five armor slots.
pub const LEFT_SLOTS: [SlotSpec; 5] = [
    slot(CharEquipSlotKind::HeadUpper, "head"),
    slot(CharEquipSlotKind::HeadMid, "headm"),
    slot(CharEquipSlotKind::HeadLower, "headl"),
    slot(CharEquipSlotKind::Body, "armor"),
    slot(CharEquipSlotKind::Garment, "garment"),
];

/// Right column, top-to-bottom: hands, accessories, footgear, ammunition.
pub const RIGHT_SLOTS: [SlotSpec; 6] = [
    slot(CharEquipSlotKind::RightHand, "sword"),
    slot(CharEquipSlotKind::LeftHand, "shield"),
    slot(CharEquipSlotKind::AccessoryRight, "ring"),
    slot(CharEquipSlotKind::AccessoryLeft, "ring"),
    slot(CharEquipSlotKind::Footgear, "boot"),
    slot(CharEquipSlotKind::Ammo, "ammo"),
];

/// Which side of the preview a column is on. Captions sit on the outer side.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

pub fn register(app: &mut App) {
    app.init_resource::<EquipUi>()
        .init_resource::<CharLastSlotClick>()
        .add_systems(
            Update,
            rebuild_equipment.run_if(in_state(GameState::InGame)),
        )
        .add_systems(OnExit(GameState::InGame), reset);
}

fn reset(mut ui: ResMut<EquipUi>) {
    *ui = EquipUi::default();
}

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested).
// ---------------------------------------------------------------------------

/// The worn item whose wear state covers `wear_bit`.
pub fn item_in_slot(inventory: &Inventory, wear_bit: u32) -> Option<&Item> {
    inventory
        .equipped()
        .find(|item| item.wear_state & wear_bit != 0)
}

/// The item in the selected slot of the open set.
fn selected_item<'a>(inventory: &'a Inventory, ui: &EquipUi) -> Option<&'a Item> {
    let (wear_bit, _) = ui.set.slot(ui.selected?)?;
    item_in_slot(inventory, wear_bit)
}

/// How many slots of `set` hold an item.
fn worn_count(inventory: &Inventory, set: EquipSet) -> usize {
    LEFT_SLOTS
        .iter()
        .chain(RIGHT_SLOTS.iter())
        .filter_map(|spec| set.slot(spec.kind))
        .filter(|(wear_bit, _)| item_in_slot(inventory, *wear_bit).is_some())
        .count()
}

/// The selection bar's card line: the socketed cards with repeats folded, then the
/// free sockets.
fn cards_line(cards: &[String], free: u8) -> String {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for card in cards {
        match counts.iter_mut().find(|(name, _)| *name == card) {
            Some((_, count)) => *count += 1,
            None => counts.push((card, 1)),
        }
    }
    let mut parts = Vec::new();
    if !counts.is_empty() {
        let names: Vec<String> = counts
            .into_iter()
            .map(|(name, count)| match count {
                1 => name.to_string(),
                _ => format!("{name} \u{d7}{count}"),
            })
            .collect();
        parts.push(names.join(", "));
    }
    match free {
        0 => {}
        1 => parts.push("1 empty slot".to_string()),
        _ => parts.push(format!("{free} empty slots")),
    }
    if parts.is_empty() {
        return "No card slots".to_string();
    }
    parts.join(" \u{b7} ")
}

/// Last slot click, for double-click detection across rebuilds.
#[derive(Resource, Default)]
pub struct CharLastSlotClick {
    index: u16,
    at: Duration,
}

fn is_slot_double_click(last: &CharLastSlotClick, index: u16, now: Duration) -> bool {
    last.index == index && now.saturating_sub(last.at) <= DOUBLE_CLICK
}

// ---------------------------------------------------------------------------
// Static frame.
// ---------------------------------------------------------------------------

/// The tabs over the paperdoll panel, the selection bar and the trait read-out.
pub fn equipment_column() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [
            EquipMount::Tabs
            Node { column_gap: px(4) }
            @ignore_picking()
            --
            @inset()
            Node {
                padding: px(12),
                column_gap: px(15),
                border_radius: {BorderRadius::all(px(6)).with_top_left(px(0))},
            }
            @ignore_picking()
            Children [
                EquipMount::Left
                @slot_column()
                --
                @preview::stage()
                --
                EquipMount::Right
                @slot_column()
            ]
            --
            EquipMount::Selection
            @inset()
            Node {
                height: px(64),
                margin: {UiRect::top(px(12))},
                padding: {UiRect::new(px(10), px(12), px(0), px(0))},
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: px(12),
            }
            @ignore_picking()
            --
            @attributes::trait_combat_panel()
        ]
    }
}

fn slot_column() -> impl Scene {
    bsn! {
        Node {
            width: px(SLOT_COLUMN),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
        }
        @ignore_picking()
    }
}

// ---------------------------------------------------------------------------
// Rebuilt content.
// ---------------------------------------------------------------------------

/// Respawns the tabs, both slot columns and the selection bar on every inventory or
/// [`EquipUi`] change, when the item catalog loads, and once when the mounts appear.
pub fn rebuild_equipment(
    mut commands: Commands,
    inventory: Res<Inventory>,
    ui: Res<EquipUi>,
    item_db: Option<Res<ItemDb>>,
    mounts: Query<(Entity, &EquipMount, Option<&Children>)>,
    added: Query<(), Added<EquipMount>>,
) {
    let catalog_loaded = item_db.as_ref().is_some_and(|db| db.is_changed());
    if !inventory.is_changed() && !ui.is_changed() && !catalog_loaded && added.is_empty() {
        return;
    }
    let db = item_db.as_deref();
    for (mount, kind, children) in &mounts {
        for child in children.into_iter().flatten() {
            commands.entity(*child).despawn();
        }
        let content = match kind {
            EquipMount::Tabs => tabs(&inventory, ui.set),
            EquipMount::Left => slot_rows(&LEFT_SLOTS, Side::Left, &inventory, &ui, db),
            EquipMount::Right => slot_rows(&RIGHT_SLOTS, Side::Right, &inventory, &ui, db),
            EquipMount::Selection => selection(&inventory, &ui, db),
        };
        for scene in content {
            commands.spawn_scene(scene).insert(ChildOf(mount));
        }
    }
}

fn tabs(inventory: &Inventory, open: EquipSet) -> Vec<Box<dyn Scene>> {
    EquipSet::ALL
        .into_iter()
        .map(|set| {
            Box::new(set_tab(set, worn_count(inventory, set), set == open)) as Box<dyn Scene>
        })
        .collect()
}

/// A set's tab with the number of items worn in it.
fn set_tab(set: EquipSet, count: usize, active: bool) -> impl Scene {
    let (label, count_color) = tab_text(active);
    bsn! {
        EquipTabButton(set)
        @tab(active)
        on(pick_set)
        Children [
            @chrome_text(set.label().into(), 13.0, label)
            --
            @chrome_text(count.to_string(), 11.0, count_color)
        ]
    }
}

fn slot_rows(
    specs: &[SlotSpec],
    side: Side,
    inventory: &Inventory,
    ui: &EquipUi,
    item_db: Option<&ItemDb>,
) -> Vec<Box<dyn Scene>> {
    specs
        .iter()
        .map(|spec| Box::new(slot_row(*spec, side, inventory, ui, item_db)) as Box<dyn Scene>)
        .collect()
}

/// One slot with its caption: the slot's name over the worn item's. A slot the open
/// set does not use is disabled and keeps its Equipment name.
fn slot_row(
    spec: SlotSpec,
    side: Side,
    inventory: &Inventory,
    ui: &EquipUi,
    item_db: Option<&ItemDb>,
) -> impl Scene + use<> {
    let slot = ui.set.slot(spec.kind);
    let item = slot.and_then(|(wear_bit, _)| item_in_slot(inventory, wear_bit));
    let selected = item
        .filter(|_| ui.selected == Some(spec.kind))
        .map(|item| item.index);
    let cell = item.map(|item| cell_view(item, item_db, selected));
    let rare = cell.as_ref().is_some_and(|cell| cell.ring == Ring::Rare);

    let (label, label_color) = match slot {
        Some((_, label)) => (label, ro_dark::ACCENT),
        None => (spec.kind.slot_label(), ro_dark::FAINT),
    };
    let (name, name_color) = match (item, slot) {
        (Some(item), _) if rare => (item_name(item, item_db), ro_dark::RARE_RING),
        (Some(item), _) => (item_name(item, item_db), ro_dark::BODY),
        (None, Some(_)) => ("Empty".to_string(), ro_dark::FAINT),
        (None, None) => ("Not used here".to_string(), ro_dark::FAINT),
    };
    let well: Box<dyn Scene> = match cell {
        Some(cell) => Box::new(filled_slot(spec.kind, cell)),
        None => Box::new(empty_slot(spec.glyph, slot.is_some())),
    };
    let direction = match side {
        Side::Left => FlexDirection::RowReverse,
        Side::Right => FlexDirection::Row,
    };
    bsn! {
        Node {
            height: px(SLOT),
            flex_direction: {direction},
            align_items: AlignItems::Center,
            column_gap: px(8),
        }
        @ignore_picking()
        Children [
            @well
            --
            @caption(label, label_color, name, name_color, side)
        ]
    }
}

/// The two caption lines, aligned towards the slot. The name wraps to two lines and
/// is cut off beyond them.
fn caption(
    label: &'static str,
    label_color: Color,
    name: String,
    name_color: Color,
    side: Side,
) -> impl Scene {
    let (align, justify) = match side {
        Side::Left => (AlignItems::FlexEnd, Justify::Right),
        Side::Right => (AlignItems::FlexStart, Justify::Left),
    };
    bsn! {
        Node {
            width: px(CAPTION),
            max_height: px(SLOT),
            flex_direction: FlexDirection::Column,
            align_items: {align},
            overflow: {Overflow::clip()},
        }
        @ignore_picking()
        Children [
            @overline(label.to_uppercase(), label_color)
            --
            @chrome_text(name, 12.0, name_color)
            TextLayout { justify: {justify} }
        ]
    }
}

/// A worn item: click to select, double-click to unequip, right-click to inspect.
fn filled_slot(kind: CharEquipSlotKind, cell: CellView) -> impl Scene {
    let (index, ring) = (cell.index, cell.ring);
    let (stroke, _, _) = ring.style();
    let hover_stroke = match ring {
        Ring::None => ro_dark::BUTTON_STROKE,
        Ring::Rare | Ring::Selected => stroke,
    };
    let colors = HoverColors {
        rest: (ro_dark::SLOT, stroke),
        hover: (ro_dark::SLOT_HOVER, hover_stroke),
    };
    let marks = slot_marks(&cell, ICON, true);
    bsn! {
        EquipSlot { kind: kind, index: index }
        Hovered
        colors
        @slot_frame(SLOT, ring)
        Pickable
        on(on_slot_click)
        Children [ {marks} ]
    }
}

/// An empty slot showing its ghosted glyph, greyed out where the set has no use for it.
fn empty_slot(glyph: &'static str, usable: bool) -> impl Scene {
    let (fill, stroke) = if usable {
        (ro_dark::SLOT, ro_dark::SLOT_STROKE)
    } else {
        (ro_dark::SLOT_DISABLED, ro_dark::INSET_RULE)
    };
    let ghost = ro_dark::LABEL.with_alpha(0.2);
    bsn! {
        @slot_frame(SLOT, Ring::None)
        BackgroundColor(fill)
        BorderColor::all(stroke)
        @ignore_picking()
        Children [ @glyph_icon(glyph, 30.0, ghost) ]
    }
}

/// The bar under the paperdoll: the selected item with its Unequip button, or a hint.
fn selection(inventory: &Inventory, ui: &EquipUi, item_db: Option<&ItemDb>) -> Vec<Box<dyn Scene>> {
    let Some(item) = selected_item(inventory, ui) else {
        return vec![Box::new(chrome_text(
            "Select a slot \u{b7} Double-click to unequip \u{b7} Right-click for details".into(),
            12.0,
            ro_dark::MUTED,
        ))];
    };
    let cell = cell_view(item, item_db, None);
    let ring = cell.ring;
    let name_color = match ring {
        Ring::Rare => ro_dark::RARE_RING,
        _ => ro_dark::STRONG,
    };
    let slot_name = ui
        .selected
        .and_then(|kind| ui.set.slot(kind))
        .map_or("", |(_, label)| label);
    let cards = item_db.map_or_else(String::new, |db| {
        let info = view::inventory_item_view(item, db, None);
        cards_line(&info.cards, info.sockets_total - info.sockets_filled)
    });
    let marks = slot_marks(&cell, SELECTION_ICON, false);
    vec![
        Box::new(bsn! {
            @slot_frame(SELECTION_SLOT, ring)
            @ignore_picking()
            Children [ {marks} ]
        }),
        Box::new(bsn! {
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                overflow: {Overflow::clip()},
            }
            @ignore_picking()
            Children [
                @chrome_text(item_name(item, item_db), 13.0, name_color)
                TextLayout { linebreak: LineBreak::NoWrap }
                --
                @chrome_text(slot_name.into(), 11.0, ro_dark::MUTED)
                TextLayout { linebreak: LineBreak::NoWrap }
                --
                @chrome_text(cards, 11.0, ro_dark::FROST)
                TextLayout { linebreak: LineBreak::NoWrap }
            ]
        }),
        Box::new(bsn! {
            @flush_button(ButtonArt::NORMAL, "Unequip")
            Node { width: px(88.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0, flex_shrink: 0.0 }
            on(unequip_selected)
        }),
    ]
}

// ---------------------------------------------------------------------------
// Observers.
// ---------------------------------------------------------------------------

/// A tab opens its set and drops the selection, which named a slot of the old set.
fn pick_set(click: On<PointerClick>, tabs: Query<&EquipTabButton>, mut ui: ResMut<EquipUi>) {
    let Ok(tab) = tabs.get(click.entity) else {
        return;
    };
    ui.set = tab.0;
    ui.selected = None;
}

/// A click selects the slot and a double-click unequips its item. A secondary click
/// opens the info modal instead and leaves the selection alone.
#[allow(clippy::too_many_arguments)]
fn on_slot_click(
    click: On<PointerClick>,
    slots: Query<&EquipSlot>,
    mut ui: ResMut<EquipUi>,
    time: Res<Time>,
    mut last: ResMut<CharLastSlotClick>,
    inventory: Res<Inventory>,
    item_db: Option<Res<ItemDb>>,
    option_db: Option<Res<RandomOptionDb>>,
    mut unequip: MessageWriter<UnequipItemRequested>,
    mut info_writer: MessageWriter<ShowInfoModal>,
) {
    let Ok(slot) = slots.get(click.entity) else {
        return;
    };
    if click.button == PointerButton::Secondary {
        let Some(item_db) = item_db.as_deref() else {
            warn!("equipment: ItemDb not loaded yet, ignoring inspect");
            return;
        };
        if let Some(item) = inventory.get(slot.index) {
            info_writer.write(ShowInfoModal {
                content: InfoContent::Item {
                    view: view::inventory_item_view(item, item_db, option_db.as_deref()),
                    action: Some(ItemAction::unequip(slot.index)),
                },
            });
        }
        return;
    }
    let now = time.elapsed();
    if is_slot_double_click(&last, slot.index, now) {
        unequip.write(UnequipItemRequested { index: slot.index });
    }
    *last = CharLastSlotClick {
        index: slot.index,
        at: now,
    };
    if ui.selected != Some(slot.kind) {
        ui.selected = Some(slot.kind);
    }
}

/// The selection bar's button unequips the selected item.
fn unequip_selected(
    _: On<Activate>,
    ui: Res<EquipUi>,
    inventory: Res<Inventory>,
    mut unequip: MessageWriter<UnequipItemRequested>,
) {
    let Some(item) = selected_item(&inventory, &ui) else {
        return;
    };
    unequip.write(UnequipItemRequested { index: item.index });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;

    fn equipped(index: u16, wear_state: u32) -> Item {
        Item {
            index,
            wear_state,
            amount: 1,
            ..Default::default()
        }
    }

    #[test]
    fn wear_bit_matches_location_constant() {
        let pairs = [
            (CharEquipSlotKind::HeadUpper, EQP_HEAD_TOP),
            (CharEquipSlotKind::HeadMid, EQP_HEAD_MID),
            (CharEquipSlotKind::HeadLower, EQP_HEAD_LOW),
            (CharEquipSlotKind::Body, EQP_ARMOR),
            (CharEquipSlotKind::Garment, EQP_GARMENT),
            (CharEquipSlotKind::RightHand, EQP_RIGHT_HAND),
            (CharEquipSlotKind::LeftHand, EQP_LEFT_HAND),
            (CharEquipSlotKind::AccessoryRight, EQP_RIGHT_ACCESSORY),
            (CharEquipSlotKind::AccessoryLeft, EQP_LEFT_ACCESSORY),
            (CharEquipSlotKind::Footgear, EQP_SHOES),
        ];
        for (kind, bit) in pairs {
            assert_eq!(kind.wear_bit(), bit, "{kind:?}");
        }
    }

    #[test]
    fn item_in_slot_returns_item_for_filled_slot() {
        let mut inv = Inventory::default();
        inv.upsert(equipped(2, EQP_ARMOR));

        let item = item_in_slot(&inv, EQP_ARMOR).expect("armor in body slot");

        assert_eq!(item.index, 2);
    }

    #[test]
    fn item_in_slot_returns_none_for_empty_slot() {
        let mut inv = Inventory::default();
        inv.upsert(equipped(2, EQP_ARMOR));

        assert!(item_in_slot(&inv, EQP_SHOES).is_none());
    }

    #[test]
    fn two_handed_weapon_resolves_into_both_hand_slots() {
        let mut inv = Inventory::default();
        inv.upsert(equipped(3, EQP_RIGHT_HAND | EQP_LEFT_HAND));

        let right = item_in_slot(&inv, EQP_RIGHT_HAND).expect("right hand");
        let left = item_in_slot(&inv, EQP_LEFT_HAND).expect("left hand");

        assert_eq!(right.index, 3);
        assert_eq!(left.index, 3);
    }

    #[test]
    fn every_slot_kind_has_a_label() {
        for spec in LEFT_SLOTS.iter().chain(RIGHT_SLOTS.iter()) {
            assert!(!spec.kind.slot_label().is_empty(), "{:?}", spec.kind);
        }
    }

    #[test]
    fn slot_table_covers_every_equip_slot_kind() {
        let kinds: Vec<CharEquipSlotKind> = LEFT_SLOTS
            .iter()
            .chain(RIGHT_SLOTS.iter())
            .map(|spec| spec.kind)
            .collect();
        assert_eq!(kinds.len(), 11);
        let expected = [
            CharEquipSlotKind::HeadUpper,
            CharEquipSlotKind::HeadMid,
            CharEquipSlotKind::HeadLower,
            CharEquipSlotKind::Body,
            CharEquipSlotKind::Garment,
            CharEquipSlotKind::RightHand,
            CharEquipSlotKind::LeftHand,
            CharEquipSlotKind::AccessoryRight,
            CharEquipSlotKind::AccessoryLeft,
            CharEquipSlotKind::Footgear,
            CharEquipSlotKind::Ammo,
        ];
        for kind in expected {
            assert!(kinds.contains(&kind), "missing {kind:?}");
        }
    }

    #[test]
    fn slot_double_click_within_window_is_true() {
        let last = CharLastSlotClick {
            index: 3,
            at: Duration::from_millis(100),
        };
        assert!(is_slot_double_click(&last, 3, Duration::from_millis(350)));
        assert!(!is_slot_double_click(&last, 4, Duration::from_millis(350)));
        assert!(!is_slot_double_click(&last, 3, Duration::from_millis(500)));
    }

    #[test]
    fn sets_map_their_slots_to_their_own_wear_bits() {
        assert_eq!(
            EquipSet::Equipment.slot(CharEquipSlotKind::Ammo),
            Some((EQP_AMMO, "Ammunition"))
        );
        assert_eq!(
            EquipSet::Costume.slot(CharEquipSlotKind::HeadUpper),
            Some((EQP_COSTUME_HEAD_TOP, "Costume Upper"))
        );
        assert_eq!(EquipSet::Costume.slot(CharEquipSlotKind::Body), None);
        assert_eq!(
            EquipSet::Shadow.slot(CharEquipSlotKind::Body),
            Some((EQP_SHADOW_ARMOR, "Shadow Armor"))
        );
        assert_eq!(EquipSet::Shadow.slot(CharEquipSlotKind::HeadUpper), None);
    }

    #[test]
    fn worn_count_only_counts_the_sets_own_items() {
        let mut inv = Inventory::default();
        inv.upsert(equipped(2, EQP_ARMOR));
        inv.upsert(equipped(3, EQP_RIGHT_HAND | EQP_LEFT_HAND));
        inv.upsert(equipped(4, EQP_COSTUME_HEAD_TOP));

        assert_eq!(worn_count(&inv, EquipSet::Equipment), 3);
        assert_eq!(worn_count(&inv, EquipSet::Costume), 1);
        assert_eq!(worn_count(&inv, EquipSet::Shadow), 0);
    }

    #[test]
    fn cards_line_folds_repeats_and_counts_free_sockets() {
        let hydras = ["Hydra Card".to_string(), "Hydra Card".to_string()];
        assert_eq!(
            cards_line(&hydras, 1),
            "Hydra Card \u{d7}2 \u{b7} 1 empty slot"
        );
        assert_eq!(cards_line(&hydras[..1], 0), "Hydra Card");
        assert_eq!(cards_line(&[], 2), "2 empty slots");
        assert_eq!(cards_line(&[], 0), "No card slots");
    }

    fn armor_db() -> ItemDb {
        use lifthrasir_data::{ItemData, ItemInfo};
        let mut data = ItemData::default();
        data.items.insert(
            2301,
            ItemInfo {
                identified_name: "Cotton Shirt".to_string(),
                identified_resource: "COTTON_SHIRT".to_string(),
                ..Default::default()
            },
        );
        ItemDb::from_item_data(data)
    }

    fn slot_kinds(app: &mut App) -> Vec<(CharEquipSlotKind, u16)> {
        let world = app.world_mut();
        world
            .query::<&EquipSlot>()
            .iter(world)
            .map(|slot| (slot.kind, slot.index))
            .collect()
    }

    fn has_text(app: &mut App, needle: &str) -> bool {
        let world = app.world_mut();
        world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0 == needle)
    }

    #[test]
    fn rebuild_fills_then_empties_a_slot() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin))
            .init_asset::<Image>()
            .init_asset::<Font>();
        app.insert_resource(armor_db());
        app.init_resource::<EquipUi>();

        let mut inv = Inventory::default();
        inv.upsert(Item {
            index: 4,
            item_id: 2301,
            wear_state: EQP_ARMOR,
            refine: 3,
            identified: true,
            amount: 1,
            ..Default::default()
        });
        app.insert_resource(inv);
        app.world_mut().spawn(EquipMount::Left);

        app.add_systems(Update, rebuild_equipment);
        app.update();

        assert_eq!(slot_kinds(&mut app), vec![(CharEquipSlotKind::Body, 4)]);
        assert!(has_text(&mut app, "+3"), "refine badge");
        assert!(has_text(&mut app, "+3 Cotton Shirt"), "item name caption");

        app.world_mut()
            .resource_mut::<Inventory>()
            .set_wear_state(4, 0);
        app.update();

        assert!(slot_kinds(&mut app).is_empty());
        assert!(!has_text(&mut app, "+3 Cotton Shirt"));
    }

    #[test]
    fn a_set_without_a_slot_shows_it_unused() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin))
            .init_asset::<Image>()
            .init_asset::<Font>();
        app.insert_resource(EquipUi {
            set: EquipSet::Costume,
            selected: None,
        });
        let mut inv = Inventory::default();
        inv.upsert(equipped(4, EQP_ARMOR));
        app.insert_resource(inv);
        app.world_mut().spawn(EquipMount::Left);

        app.add_systems(Update, rebuild_equipment);
        app.update();

        assert!(
            slot_kinds(&mut app).is_empty(),
            "body armor is not a costume"
        );
        assert!(has_text(&mut app, "Not used here"));
        assert!(has_text(&mut app, "COSTUME UPPER"));
        let world = app.world_mut();
        let fills: Vec<Color> = world
            .query::<&BackgroundColor>()
            .iter(world)
            .map(|fill| fill.0)
            .collect();
        assert!(
            fills.contains(&ro_dark::SLOT_DISABLED),
            "unused slots are greyed"
        );
        assert!(fills.contains(&ro_dark::SLOT), "costume slots stay usable");
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

    fn slot_click_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<UnequipItemRequested>();
        app.add_message::<ShowInfoModal>();
        app.init_resource::<CharLastSlotClick>();
        app.init_resource::<EquipUi>();
        app.init_resource::<Time>();
        app.init_resource::<ItemDb>();
        app.init_resource::<Inventory>();
        app
    }

    fn body_slot(index: u16) -> EquipSlot {
        EquipSlot {
            kind: CharEquipSlotKind::Body,
            index,
        }
    }

    #[test]
    fn secondary_click_on_a_filled_slot_opens_the_info_modal() {
        let mut app = slot_click_app();
        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 4,
            item_id: 2104,
            identified: true,
            ..Default::default()
        });
        let slot = app
            .world_mut()
            .spawn(body_slot(4))
            .observe(on_slot_click)
            .id();
        let window = app.world_mut().spawn_empty().id();

        app.world_mut()
            .trigger(click_event(slot, window, PointerButton::Secondary));

        let messages = app.world().resource::<Messages<ShowInfoModal>>();
        let mut reader = messages.get_cursor();
        let contents: Vec<_> = reader.read(messages).map(|m| m.content.clone()).collect();
        assert_eq!(contents.len(), 1);
        assert!(matches!(
            &contents[0],
            InfoContent::Item { view, action: Some(action) }
                if view.item_id == 2104 && *action == ItemAction::unequip(4)
        ));
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<UnequipItemRequested>>()
                .drain()
                .count(),
            0
        );
        assert_eq!(app.world().resource::<EquipUi>().selected, None);
    }

    #[test]
    fn secondary_click_on_an_empty_slot_writes_nothing() {
        let mut app = slot_click_app();
        let slot = app.world_mut().spawn_empty().observe(on_slot_click).id();
        let window = app.world_mut().spawn_empty().id();

        app.world_mut()
            .trigger(click_event(slot, window, PointerButton::Secondary));

        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<ShowInfoModal>>()
                .drain()
                .count(),
            0
        );
    }

    #[test]
    fn primary_click_selects_and_a_double_click_unequips() {
        let mut app = slot_click_app();
        let slot = app
            .world_mut()
            .spawn(body_slot(4))
            .observe(on_slot_click)
            .id();
        let window = app.world_mut().spawn_empty().id();

        app.world_mut()
            .trigger(click_event(slot, window, PointerButton::Primary));
        assert_eq!(
            app.world().resource::<EquipUi>().selected,
            Some(CharEquipSlotKind::Body)
        );
        assert_eq!(
            app.world()
                .resource::<Messages<UnequipItemRequested>>()
                .len(),
            0
        );

        app.world_mut()
            .trigger(click_event(slot, window, PointerButton::Primary));

        let unequips: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<UnequipItemRequested>>()
            .drain()
            .collect();
        assert_eq!(unequips.len(), 1);
        assert_eq!(unequips[0].index, 4);
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<ShowInfoModal>>()
                .drain()
                .count(),
            0
        );
    }

    #[test]
    fn the_unequip_button_acts_on_the_selected_slot_of_the_open_set() {
        let mut app = slot_click_app();
        app.world_mut()
            .resource_mut::<Inventory>()
            .upsert(equipped(7, EQP_SHADOW_ARMOR));
        app.insert_resource(EquipUi {
            set: EquipSet::Shadow,
            selected: Some(CharEquipSlotKind::Body),
        });
        let button = app.world_mut().spawn_empty().observe(unequip_selected).id();

        app.world_mut().trigger(Activate { entity: button });

        let unequips: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<UnequipItemRequested>>()
            .drain()
            .collect();
        assert_eq!(unequips.len(), 1);
        assert_eq!(unequips[0].index, 7);
    }

    #[test]
    fn picking_a_tab_opens_its_set_and_drops_the_selection() {
        let mut app = slot_click_app();
        app.insert_resource(EquipUi {
            set: EquipSet::Equipment,
            selected: Some(CharEquipSlotKind::Body),
        });
        let tab = app
            .world_mut()
            .spawn(EquipTabButton(EquipSet::Costume))
            .observe(pick_set)
            .id();
        let window = app.world_mut().spawn_empty().id();

        app.world_mut()
            .trigger(click_event(tab, window, PointerButton::Primary));

        let ui = app.world().resource::<EquipUi>();
        assert_eq!(ui.set, EquipSet::Costume);
        assert_eq!(ui.selected, None);
    }
}
