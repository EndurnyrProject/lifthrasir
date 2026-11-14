//! What the hotbar shows, as plain values: what fills each slot and the state it is
//! in, and the tooltip, picker rows and toast words made from that. No ECS in here.

use std::time::Duration;

use bevy::prelude::*;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::catalogs::skill::registry::SkillCatalog;
use lifthrasir_assets::paths::item_icon_path;
use lifthrasir_client::domain::hotbar::{Hotbar, HotbarSlot, PAGES};
use lifthrasir_client::domain::inventory::{Inventory, ItemCategory};
use lifthrasir_client::domain::skill::{Form, SkillCastLevels, SkillTreeState, form};
use lifthrasir_ui::theme::ro_dark;

use crate::screens::shared::{clock, glow, seconds, thousands};
use crate::widgets::skills_window::skill_name;

/// Ink of SP, as a label and as a cost the hero can pay.
pub(super) const SP_INK: Color = Color::srgb_u8(0x9c, 0xc0, 0xe6);
/// Ink of what the hero is short of: an empty stack, a cost out of reach.
pub(super) const SHORT_INK: Color = Color::srgb_u8(0xe8, 0x89, 0x7e);

/// What the slots are read from.
pub(super) struct Facts<'a> {
    pub inventory: &'a Inventory,
    pub catalog: Option<&'a SkillCatalog>,
    pub items: Option<&'a ItemDb>,
    pub tree: &'a SkillTreeState,
    pub cast_levels: &'a SkillCastLevels,
    /// SP the hero has left.
    pub sp: u32,
}

/// What changes about a skill's slot while the skill is in use.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Live {
    /// Seconds until the skill is ready again.
    pub cooldown: Option<f32>,
    /// Time left on the status the skill put on the hero.
    pub buff: Option<Duration>,
    pub casting: bool,
}

/// What fills a slot and the state it is in.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Filled {
    pub slot: HotbarSlot,
    pub name: String,
    pub icon: Option<String>,
    /// Items: how many are in the bag.
    pub count: Option<u32>,
    /// Skills: the level it is cast at. `None` when it is not learned.
    pub level: Option<u32>,
    /// Skills: the SP a cast costs, when it costs any.
    pub sp: Option<u32>,
    pub lacks_sp: bool,
    /// Learned skills: the base cast time and the cooldown at the cast level, in
    /// milliseconds. Zero is an instant cast, or no cooldown.
    pub timing: Option<(u32, u32)>,
    /// Time until the skill is ready again, as drawn over its icon.
    pub cooldown: Option<String>,
    /// Time left on the status the skill gave, as drawn in the slot's corner.
    pub buff: Option<String>,
    pub casting: bool,
}

impl Filled {
    pub(super) fn is_skill(&self) -> bool {
        matches!(self.slot, HotbarSlot::Skill(_))
    }

    /// Whether using it now would do nothing: none left, not learned or too little SP.
    pub(super) fn spent(&self) -> bool {
        self.lacks_sp || self.count == Some(0) || (self.is_skill() && self.level.is_none())
    }
}

/// One slot of the bar.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SlotView {
    /// The key that uses the slot, as drawn in its corner. Empty when none is bound.
    pub key: String,
    pub filled: Option<Filled>,
}

pub(super) fn slot_view(
    slot: Option<HotbarSlot>,
    key: String,
    facts: &Facts,
    live: Live,
) -> SlotView {
    SlotView {
        key,
        filled: slot.map(|slot| filled(slot, facts, live)),
    }
}

pub(super) fn filled(slot: HotbarSlot, facts: &Facts, live: Live) -> Filled {
    match slot {
        HotbarSlot::Skill(id) => skill(id, facts, live),
        HotbarSlot::Item(id) => item(id, facts),
    }
}

fn skill(id: u32, facts: &Facts, live: Live) -> Filled {
    let node = facts.tree.skills.get(&id).filter(|node| node.level > 0);
    let level = node.map(|node| facts.cast_levels.level(id, node.level));
    let listed = |level: u32| {
        let costs = &facts.catalog?.get(id)?.sp_cost;
        costs.get(level.saturating_sub(1) as usize).copied()
    };
    let sp = level
        .and_then(|level| listed(level).map(u32::from).or(node.map(|node| node.sp)))
        .filter(|&sp| sp > 0);
    Filled {
        slot: HotbarSlot::Skill(id),
        name: skill_name(id, facts.catalog),
        icon: facts.catalog.and_then(|catalog| catalog.icon_path(id)),
        count: None,
        level,
        sp,
        lacks_sp: sp.is_some_and(|sp| sp > facts.sp),
        timing: node
            .zip(level)
            .map(|(node, level)| (node.cast_time_at(level), node.cooldown_at(level))),
        cooldown: live.cooldown.map(cooldown_text),
        buff: live.buff.map(clock),
        casting: live.casting,
    }
}

/// An item the bag no longer holds keeps its slot, drawn with its identified art.
fn item(id: u32, facts: &Facts) -> Filled {
    let held = facts.inventory.iter().find(|item| item.item_id == id);
    let identified = held.is_none_or(|item| item.identified);
    Filled {
        slot: HotbarSlot::Item(id),
        name: facts
            .items
            .and_then(|db| db.name(id, identified))
            .map_or_else(|| format!("Item #{id}"), str::to_string),
        icon: facts
            .items
            .and_then(|db| db.icon_resource(id, identified))
            .map(item_icon_path),
        count: Some(held.map_or(0, |item| item.amount.into())),
        level: None,
        sp: None,
        lacks_sp: false,
        timing: None,
        cooldown: None,
        buff: None,
        casting: false,
    }
}

/// Tenths in the last second, whole seconds before it.
fn cooldown_text(secs: f32) -> String {
    if secs < 1.0 {
        return format!("{secs:.1}");
    }
    format!("{}", secs.ceil() as u32)
}

/// Whether the slot's skill just came off cooldown between two views of it.
pub(super) fn cooled_down(before: Option<&SlotView>, now: &SlotView) -> bool {
    let (Some(before), Some(now)) = (
        before.and_then(|view| view.filled.as_ref()),
        now.filled.as_ref(),
    ) else {
        return false;
    };
    before.slot == now.slot && before.cooldown.is_some() && now.cooldown.is_none()
}

/// Why using the slot now does nothing, as the toast says it. `casting` names the
/// skill being cast.
pub(super) fn refusal(filled: &Filled, casting: Option<&str>) -> Option<String> {
    let name = &filled.name;
    if !filled.is_skill() {
        return (filled.count == Some(0)).then(|| format!("You have no {name} left"));
    }
    if filled.level.is_none() {
        return Some(format!("{name} is not learned"));
    }
    if let Some(left) = &filled.cooldown {
        return Some(format!("{name} is ready in {left}s"));
    }
    if let Some(casting) = casting {
        return Some(format!("Already casting {casting}"));
    }
    filled
        .sp
        .filter(|_| filled.lacks_sp)
        .map(|sp| format!("Not enough SP \u{b7} {name} needs {sp}"))
}

/// A slot's outline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Edge {
    Rest,
    Hover,
    Empty,
    EmptyHover,
    /// The slot's skill has a status running on the hero.
    Active,
    /// A drag would drop here.
    DropTarget,
}

pub(super) fn edge(view: &SlotView, hovered: bool, drop_target: bool) -> Edge {
    if drop_target {
        return Edge::DropTarget;
    }
    match (&view.filled, hovered) {
        (Some(filled), _) if filled.buff.is_some() => Edge::Active,
        (Some(_), true) => Edge::Hover,
        (Some(_), false) => Edge::Rest,
        (None, true) => Edge::EmptyHover,
        (None, false) => Edge::Empty,
    }
}

impl Edge {
    /// Stroke colour and halo.
    pub(super) fn style(self) -> (Color, BoxShadow) {
        let (stroke, halo) = match self {
            Self::Rest => (ro_dark::SLOT_STROKE, Color::NONE),
            Self::Hover => (ro_dark::PRIMARY_STROKE, Color::NONE),
            Self::Empty => (ro_dark::SLOT_STROKE.with_alpha(0.5), Color::NONE),
            Self::EmptyHover => (ro_dark::FROST_GLOW, Color::NONE),
            Self::Active => (ro_dark::RARE_RING, ro_dark::RARE_RING.with_alpha(0.55)),
            Self::DropTarget => (ro_dark::FROST_GLOW, ro_dark::FROST_GLOW.with_alpha(0.5)),
        };
        (stroke, glow(halo))
    }
}

/// The tooltip of a filled slot.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct TipView {
    pub name: String,
    pub tone: Color,
    /// What the slot holds, in small capitals beside the name.
    pub tag: &'static str,
    /// Label, value and the value's ink.
    pub stats: Vec<(&'static str, String, Color)>,
    pub hint: String,
}

// NOTE: the design also has a one-line description and an AP cost. The client has no
// short descriptions and the server no AP costs, so they are left out. The cast time
// is the server's base one, before DEX, INT, equipment and statuses shorten it.
pub(super) fn tip(view: &SlotView, locked: bool) -> Option<TipView> {
    let filled = view.filled.as_ref()?;
    let mut stats = Vec::new();
    if let Some(level) = filled.level {
        stats.push(("Level", level.to_string(), ro_dark::STRONG));
    }
    if let Some(sp) = filled.sp {
        let ink = if filled.lacks_sp { SHORT_INK } else { SP_INK };
        stats.push(("SP", sp.to_string(), ink));
    }
    if let Some((cast, cooldown)) = filled.timing {
        let cast = match cast {
            0 => "Instant".to_string(),
            cast => seconds(cast),
        };
        stats.push(("Cast", cast, ro_dark::STRONG));
        if cooldown > 0 {
            stats.push(("Cooldown", seconds(cooldown), ro_dark::STRONG));
        }
    }
    if let Some(left) = &filled.buff {
        stats.push(("Lasts", left.clone(), ro_dark::BRONZE_LIGHT));
    }
    if let Some(count) = filled.count {
        let ink = if count == 0 {
            SHORT_INK
        } else {
            ro_dark::STRONG
        };
        stats.push(("In bag", thousands(count.into()), ink));
    }
    let active = filled.buff.is_some();
    let tag = match filled.slot {
        HotbarSlot::Skill(_) if active => "ACTIVE",
        HotbarSlot::Skill(_) => "SKILL",
        HotbarSlot::Item(_) => "ITEM",
    };
    let press = if view.key.is_empty() {
        "Click".to_string()
    } else {
        format!("Press {} or click", view.key)
    };
    let edit = if locked {
        ""
    } else {
        " \u{b7} Right-click to remove \u{b7} Drag to move"
    };
    Some(TipView {
        name: filled.name.clone(),
        tone: if active {
            ro_dark::RARE_RING
        } else {
            ro_dark::STRONG
        },
        tag,
        stats,
        hint: format!("{press}{edit}"),
    })
}

/// Which list the picker shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum PickTab {
    #[default]
    Skills,
    Items,
}

impl PickTab {
    pub(super) const ALL: [Self; 2] = [Self::Skills, Self::Items];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Skills => "Skills",
            Self::Items => "Items",
        }
    }
}

/// One row of the picker.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Choice {
    pub slot: HotbarSlot,
    pub name: String,
    pub icon: Option<String>,
    /// The level or the stack, or that the bar already has it.
    pub meta: String,
    pub on_bar: bool,
}

/// What an empty slot can be given: the learned skills that are cast, or the usable
/// items in the bag. One already on the bar is listed but cannot be picked again.
pub(super) fn choices(tab: PickTab, hotbar: &Hotbar, facts: &Facts) -> Vec<Choice> {
    let mut slots: Vec<(u32, HotbarSlot)> = match tab {
        PickTab::Skills => facts
            .tree
            .skills
            .iter()
            .filter(|(_, node)| node.level > 0 && form(node.inf_type) != Form::Passive)
            .map(|(&id, _)| (id, HotbarSlot::Skill(id)))
            .collect(),
        PickTab::Items => facts
            .inventory
            .iter()
            .filter(|item| item.category() == ItemCategory::Use && !item.is_equipped())
            .map(|item| (item.item_id, HotbarSlot::Item(item.item_id)))
            .collect(),
    };
    slots.sort_by_key(|&(id, _)| id);
    slots.dedup_by_key(|&mut (id, _)| id);
    slots
        .into_iter()
        .map(|(_, slot)| {
            let filled = filled(slot, facts, Live::default());
            let on_bar = hotbar.contains(slot);
            let meta = match (on_bar, filled.count, filled.level) {
                (true, _, _) => "On bar".to_string(),
                (_, Some(count), _) => format!("\u{d7}{}", thousands(count.into())),
                (_, _, Some(level)) => format!("Lv {level}"),
                _ => String::new(),
            };
            Choice {
                slot,
                name: filled.name,
                icon: filled.icon,
                meta,
                on_bar,
            }
        })
        .collect()
}

/// The pages on show, between the pager's arrows: `1·2` for two rows, `1 / 3` for one.
pub(super) fn page_label(hotbar: &Hotbar) -> String {
    let first = hotbar.row_page(0) + 1;
    if hotbar.two_rows {
        return format!("{first}\u{b7}{}", hotbar.row_page(1) + 1);
    }
    format!("{first} / {PAGES}")
}

/// The mood of a toast.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Tone {
    #[default]
    Info,
    Warn,
    Good,
}

impl Tone {
    /// Ink and outline.
    pub(super) fn style(self) -> (Color, Color) {
        match self {
            Self::Info => (ro_dark::BODY, ro_dark::BUTTON_STROKE),
            Self::Warn => (ro_dark::BRONZE_LIGHT, ro_dark::STATUS_BUSY.with_alpha(0.7)),
            Self::Good => (
                Color::srgb_u8(0xa9, 0xd3, 0x94),
                ro_dark::STATUS_ONLINE.with_alpha(0.7),
            ),
        }
    }
}

/// What the toast over the bar says.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Notice {
    /// Why a used slot did nothing.
    Refused(String),
    /// The bar is locked and was asked to change.
    Locked,
    /// The lock was turned on or off.
    LockSet(bool),
    /// A slot was emptied. The cell is `None` when it was dragged off the bar.
    Removed(HotbarSlot, Option<(usize, usize)>),
    /// The picker filled the slot at this cell.
    Placed(HotbarSlot, (usize, usize)),
}

impl Notice {
    /// The skill or item and the cell the words name, if any.
    pub(super) fn subject(&self) -> Option<(HotbarSlot, Option<(usize, usize)>)> {
        match *self {
            Self::Removed(slot, cell) => Some((slot, cell)),
            Self::Placed(slot, cell) => Some((slot, Some(cell))),
            _ => None,
        }
    }

    pub(super) fn tone(&self) -> Tone {
        match self {
            Self::Refused(_) | Self::Locked => Tone::Warn,
            Self::Placed(..) => Tone::Good,
            Self::LockSet(_) | Self::Removed(..) => Tone::Info,
        }
    }

    /// The words, given the subject's `name` and its cell's `key` (empty when the
    /// cell has no key or there is no cell).
    pub(super) fn text(&self, name: &str, key: &str) -> String {
        match self {
            Self::Refused(why) => why.clone(),
            Self::Locked => "Unlock the hotbar to change it".to_string(),
            Self::LockSet(true) => {
                "Hotbar locked \u{b7} Slots can't be moved or removed".to_string()
            }
            Self::LockSet(false) => "Hotbar unlocked".to_string(),
            Self::Removed(..) if key.is_empty() => format!("Removed {name}"),
            Self::Removed(..) => format!("Removed {name} from {key}"),
            Self::Placed(..) if key.is_empty() => format!("{name} added"),
            Self::Placed(..) => format!("{name} added to {key}"),
        }
    }
}
