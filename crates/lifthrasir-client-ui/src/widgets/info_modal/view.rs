//! View-model builders for the info modal: one per domain payload an item can come
//! from (`inventory_item_view`, `storage_item_view`, `cart_item_view`,
//! `shop_item_view`) plus [`build_skill_view`] and [`guild_skill_view`], each
//! producing a plain [`ItemInfoView`]/[`SkillInfoView`]. The summoning surface calls
//! the builder for the data it holds; the modal never resolves ids itself. Every
//! optional section is `Option`/`Vec` so the scenes can skip absent sections
//! without special-casing.
//!
//! These builders are pure functions over borrowed references — no `Res<T>`/ECS
//! access — so they are unit-testable without spinning up an `App`.

use bevy::prelude::Color;

use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::catalogs::random_option::RandomOptionDb;
use lifthrasir_assets::catalogs::skill::registry::SkillCatalog;
use lifthrasir_assets::paths::{item_collection_path, item_icon_path};
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::equipment::decode_wear_location;
use lifthrasir_client::domain::inventory::item::item_category;
use lifthrasir_client::domain::inventory::{Item, ItemCategory};
use lifthrasir_client::domain::skill::{SkillNode, SkillTreeState};
use lifthrasir_runtime::actors::EquipmentSlot;
use net_contract::dto::{
    CartItem, GuildSkillInfo, StorageItem, VendingCreatorKind, VendingListing,
};

use crate::widgets::character_window::{SkillPanelStaging, skill_name};
use lifthrasir_ui::rich_text::parse_color_codes;

use crate::widgets::info_modal::shell::EdgeGrade;

/// One description line split into `^RRGGBB`-colored runs (already parsed —
/// scenes render each run as-is, no further `parse_color_codes` call needed).
pub type ColoredLine = Vec<(Color, String)>;

/// The item modal's content, section by section. `None`/empty fields mean the
/// section is absent for this item/context.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemInfoView {
    /// Carried through so the footer can revalidate the item at its stored index
    /// is still the one shown.
    pub item_id: u32,
    pub icon_path: Option<String>,
    pub illustration_path: Option<String>,
    pub edge: EdgeGrade,
    pub name: String,
    pub identified: bool,
    /// Display-only server flag, shown as the footer star when `Some`. Only a
    /// domain `Item` (the bag) carries it; the storage vault, cart and shop DTOs
    /// do not, so their views have `None` and no star.
    pub favorite: Option<bool>,
    pub tags: Vec<String>,
    pub refine: Option<i32>,
    pub sockets_filled: u8,
    pub sockets_total: u8,
    pub cards: Vec<String>,
    /// Rendered random option lines, in slot order. Empty for unidentified
    /// items and for items without options.
    pub options: Vec<String>,
    pub description: Vec<ColoredLine>,
    /// Contextual `(label, value)` meta-grid rows — weight for storage/cart items,
    /// price for shop stock, empty otherwise.
    pub meta: Vec<(String, String)>,
}

/// One requirement/unlock chip: the referenced skill, the level it needs, and
/// whether that's currently met.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillReqChip {
    pub skill_id: u32,
    pub name: String,
    pub level: u32,
    pub met: bool,
}

/// The skill modal's content, section by section.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillInfoView {
    pub icon_path: Option<String>,
    pub edge: EdgeGrade,
    pub name: String,
    pub kind: String,
    pub level_line: String,
    pub description: Vec<ColoredLine>,
    pub sp_cost: Option<String>,
    pub range: Option<String>,
    pub requires: Vec<SkillReqChip>,
    pub unlocks: Vec<SkillReqChip>,
    pub can_raise: bool,
    pub points_left: u32,
}

// ---------------------------------------------------------------------------
// Item view builder.
// ---------------------------------------------------------------------------

/// An item instance stripped down to the fields the view needs, regardless of
/// which domain payload it came from.
struct ResolvedItem<'a> {
    item_id: u32,
    identified: bool,
    favorite: Option<bool>,
    refine: u8,
    cards: &'a [u32],
    options: Vec<String>,
    wear_mask: u32,
    category_label: Option<&'static str>,
    weight: Option<u32>,
    price: Option<u32>,
}

/// View for an item in the bag, whether inspected from the inventory, an equipped
/// slot, or the storage/cart/shop bag side.
pub fn inventory_item_view(
    item: &Item,
    item_db: &ItemDb,
    option_db: Option<&RandomOptionDb>,
) -> ItemInfoView {
    item_view_from_resolved(
        ResolvedItem {
            item_id: item.item_id,
            identified: item.identified,
            favorite: Some(item.favorite),
            refine: item.refine,
            cards: &item.cards,
            options: domain_option_lines(&item.options, option_db),
            wear_mask: item.location,
            category_label: Some(category_label(item.category())),
            weight: None,
            price: None,
        },
        item_db,
    )
}

/// View for an item in the storage vault; carries its weight.
pub fn storage_item_view(
    item: &StorageItem,
    item_db: &ItemDb,
    option_db: Option<&RandomOptionDb>,
) -> ItemInfoView {
    item_view_from_resolved(
        ResolvedItem {
            item_id: item.nameid,
            identified: item.identified,
            favorite: None,
            refine: item.refine as u8,
            cards: &item.cards,
            options: wire_option_lines(&item.options, option_db),
            wear_mask: item.location,
            category_label: Some(category_label(item_category(item.type_))),
            weight: Some(item.weight),
            price: None,
        },
        item_db,
    )
}

/// View for an item in the cart; carries its weight.
pub fn cart_item_view(
    item: &CartItem,
    item_db: &ItemDb,
    option_db: Option<&RandomOptionDb>,
) -> ItemInfoView {
    item_view_from_resolved(
        ResolvedItem {
            item_id: item.nameid,
            identified: item.identified,
            favorite: None,
            refine: item.refine as u8,
            cards: &item.cards,
            options: wire_option_lines(&item.options, option_db),
            wear_mask: 0,
            category_label: None,
            weight: Some(item.weight),
            price: None,
        },
        item_db,
    )
}

/// View for shop stock, always identified; carries the price when known.
pub fn shop_item_view(nameid: u32, price: Option<u32>, item_db: &ItemDb) -> ItemInfoView {
    item_view_from_resolved(
        ResolvedItem {
            item_id: nameid,
            identified: true,
            favorite: None,
            refine: 0,
            cards: &[],
            options: Vec::new(),
            wear_mask: 0,
            category_label: None,
            weight: None,
            price,
        },
        item_db,
    )
}

/// Broken-items bit of the wire item attribute byte.
const BROKEN_ATTRIBUTE: u32 = 0x01;

/// Full instance view for another player's vendor stock. It carries the price and
/// the instance's own metadata as context; it never implies bag ownership, so no
/// equip/use/compound action applies.
pub fn vending_item_view(
    item: &VendingListing,
    item_db: &ItemDb,
    option_db: Option<&RandomOptionDb>,
) -> ItemInfoView {
    let mut view = item_view_from_resolved(
        ResolvedItem {
            item_id: item.nameid,
            identified: item.identified,
            favorite: None,
            refine: item.refine as u8,
            cards: &item.cards,
            options: wire_option_lines(&item.options, option_db),
            wear_mask: 0,
            category_label: Some(category_label(item_category(item.type_))),
            weight: None,
            price: Some(item.price),
        },
        item_db,
    );

    if item.attribute & BROKEN_ATTRIBUTE != 0 {
        view.tags.push("Broken".to_string());
    }
    if let Some(grade) = enchant_grade_label(item.enchant_grade) {
        view.tags.push(grade);
    }

    let creator = match item.creator_kind {
        VendingCreatorKind::None => (None, None),
        VendingCreatorKind::Signed => (Some("Signature"), creator_text(item)),
        VendingCreatorKind::Forged => (Some("Forged by"), creator_text(item)),
        VendingCreatorKind::Unknown(code) => (Some("Creator"), Some(format!("Unknown ({code})"))),
    };
    if let (Some(label), Some(value)) = creator {
        view.meta.push((label.to_string(), value));
    }

    view
}

/// Reference enchant grades; a value with no verified name keeps its number.
fn enchant_grade_label(grade: u32) -> Option<String> {
    match grade {
        0 => None,
        1 => Some("Grade D".to_string()),
        2 => Some("Grade C".to_string()),
        3 => Some("Grade B".to_string()),
        4 => Some("Grade A".to_string()),
        other => Some(format!("Grade #{other}")),
    }
}

fn creator_text(item: &VendingListing) -> Option<String> {
    let name = item.signer_name.trim();
    match (name.is_empty(), item.creator_id) {
        (true, 0) => None,
        (true, id) => Some(format!("#{id}")),
        (false, 0) => Some(name.to_string()),
        (false, id) => Some(format!("{name} (#{id})")),
    }
}

fn category_label(category: ItemCategory) -> &'static str {
    match category {
        ItemCategory::Use => "Usable",
        ItemCategory::Equip => "Equipment",
        ItemCategory::Etc => "Etc",
    }
}

fn equip_slot_label(slot: EquipmentSlot) -> &'static str {
    match slot {
        EquipmentSlot::HeadTop => "Upper Headgear",
        EquipmentSlot::HeadMid => "Mid Headgear",
        EquipmentSlot::HeadBottom => "Lower Headgear",
        EquipmentSlot::Weapon => "Weapon",
        EquipmentSlot::Shield => "Shield",
        EquipmentSlot::Armor => "Armor",
        EquipmentSlot::Garment => "Garment",
        EquipmentSlot::Shoes => "Footgear",
        EquipmentSlot::Accessory1 | EquipmentSlot::Accessory2 => "Accessory",
    }
}

/// Refine ≥ 7 → Rare, refine ≥ 4 or socketed → Fine, else Common.
fn item_edge_grade(refine: u8, has_cards: bool) -> EdgeGrade {
    if refine >= 7 {
        EdgeGrade::Rare
    } else if refine >= 4 || has_cards {
        EdgeGrade::Fine
    } else {
        EdgeGrade::Common
    }
}

/// Card names for the filled sockets, in slot order, capped at `slot_count`.
/// A card id with no `ItemDb` entry falls back to `Card #<id>` rather than being
/// dropped, since it is still a real socketed card.
fn card_names(card_ids: &[u32], slot_count: u8, item_db: &ItemDb) -> Vec<String> {
    card_ids
        .iter()
        .take(slot_count as usize)
        .filter(|&&id| id != 0)
        .map(|&id| {
            item_db
                .name(id, true)
                .map(str::to_string)
                .unwrap_or_else(|| format!("Card #{id}"))
        })
        .collect()
}

/// Rendered random option lines, in slot order. A slot with id `0` is empty and
/// is skipped. An id with no template in the (optional) option catalog falls back
/// to the raw id and value rather than being dropped, so an older pak still shows
/// that the item is crafted.
fn option_line(
    option: &net_contract::dto::ItemOption,
    option_db: Option<&RandomOptionDb>,
) -> Option<String> {
    if option.id == 0 {
        return None;
    }
    Some(
        option_db
            .and_then(|db| db.line(option.id, option.value))
            .unwrap_or_else(|| format!("Option #{}: {}", option.id, option.value)),
    )
}

fn wire_option_lines(
    options: &[net_contract::dto::ItemOption],
    option_db: Option<&RandomOptionDb>,
) -> Vec<String> {
    options
        .iter()
        .filter_map(|option| option_line(option, option_db))
        .collect()
}

fn domain_option_lines(
    options: &[lifthrasir_client::domain::inventory::ItemOption],
    option_db: Option<&RandomOptionDb>,
) -> Vec<String> {
    options
        .iter()
        .filter_map(|option| {
            option_line(
                &net_contract::dto::ItemOption {
                    id: option.id,
                    value: option.value,
                    param: option.param,
                },
                option_db,
            )
        })
        .collect()
}

/// An `item_id` absent from `item_db` still yields a view — `Item #<id>`, no
/// description — rather than panicking.
fn item_view_from_resolved(resolved: ResolvedItem, item_db: &ItemDb) -> ItemInfoView {
    let item_id = resolved.item_id;
    let identified = resolved.identified;
    let favorite = resolved.favorite;
    let name = item_db
        .name(resolved.item_id, identified)
        .map(str::to_string)
        .unwrap_or_else(|| format!("Item #{}", resolved.item_id));
    let description = item_db
        .description(resolved.item_id, identified)
        .map(|lines| {
            lines
                .iter()
                .map(|line| parse_color_codes(line, lifthrasir_ui::theme::TEXT_DIM))
                .collect()
        })
        .unwrap_or_default();
    let resource = item_db.icon_resource(resolved.item_id, identified);
    let icon_path = resource.map(item_icon_path);
    let illustration_path = resource.map(item_collection_path);

    let (refine, sockets_filled, sockets_total, cards, options, edge) = if identified {
        let slot_count = item_db.slot_count(resolved.item_id).unwrap_or(0);
        let cards = card_names(resolved.cards, slot_count, item_db);
        let refine = (resolved.refine > 0).then_some(resolved.refine as i32);
        let edge = item_edge_grade(resolved.refine, !cards.is_empty());
        (
            refine,
            cards.len() as u8,
            slot_count,
            cards,
            resolved.options,
            edge,
        )
    } else {
        (None, 0, 0, Vec::new(), Vec::new(), EdgeGrade::Common)
    };

    let mut tags = Vec::new();
    tags.extend(resolved.category_label.map(str::to_string));
    tags.extend(
        decode_wear_location(resolved.wear_mask)
            .into_iter()
            .map(equip_slot_label)
            .map(str::to_string),
    );

    let mut meta = Vec::new();
    if let Some(weight) = resolved.weight {
        meta.push(("Weight".to_string(), weight.to_string()));
    }
    if let Some(price) = resolved.price {
        meta.push(("Price".to_string(), format!("{price}z")));
    }

    ItemInfoView {
        item_id,
        icon_path,
        illustration_path,
        edge,
        name,
        identified,
        favorite,
        tags,
        refine,
        sockets_filled,
        sockets_total,
        cards,
        options,
        description,
        meta,
    }
}

// ---------------------------------------------------------------------------
// Skill view builder.
// ---------------------------------------------------------------------------

fn kind_label(inf: u32) -> String {
    use lifthrasir_client::domain::skill::{Form, form};
    match form(inf) {
        Form::Passive => "Passive",
        Form::Active => "Active",
        Form::Supportive => "Supportive",
    }
    .to_string()
}

/// Effective level == max (and max > 0) → Rare, own requires unmet → Common,
/// else Fine.
fn skill_edge_grade(effective: u32, max_level: u32, requires_met: bool) -> EdgeGrade {
    if max_level > 0 && effective >= max_level {
        EdgeGrade::Rare
    } else if !requires_met {
        EdgeGrade::Common
    } else {
        EdgeGrade::Fine
    }
}

/// The per-level value at `effective`'s rank (1-based, index `effective - 1`),
/// clamped to the table's last entry. `None` for an empty table (no data at that
/// level, e.g. passives with no SP cost).
fn level_value<T: Copy>(levels: &[T], effective: u32) -> Option<T> {
    if levels.is_empty() {
        return None;
    }
    let last = levels.len() as u32 - 1;
    let index = effective.saturating_sub(1).min(last);
    Some(levels[index as usize])
}

/// Reverse index over every `SkillNode.requires` in the tree: the skills that
/// list `skill_id` as one of their prerequisites, sorted by id. Built fresh at
/// view time — the tree is small, no caching.
fn unlocks_for(
    skill_id: u32,
    tree: &SkillTreeState,
    staging: &SkillPanelStaging,
    catalog: Option<&SkillCatalog>,
) -> Vec<SkillReqChip> {
    let effective = staging.effective_level(skill_id, tree);
    let mut unlocks: Vec<SkillReqChip> = tree
        .skills
        .iter()
        .filter_map(|(&id, node)| {
            node.requires
                .iter()
                .find(|&&(req_id, _)| req_id == skill_id)
                .map(|&(_, level)| SkillReqChip {
                    skill_id: id,
                    name: skill_name(id, catalog),
                    level,
                    met: effective >= level,
                })
        })
        .collect();
    unlocks.sort_unstable_by_key(|chip| chip.skill_id);
    unlocks
}

fn requires_for(
    node: &SkillNode,
    tree: &SkillTreeState,
    staging: &SkillPanelStaging,
    catalog: Option<&SkillCatalog>,
) -> Vec<SkillReqChip> {
    node.requires
        .iter()
        .map(|&(req_id, level)| SkillReqChip {
            skill_id: req_id,
            name: skill_name(req_id, catalog),
            level,
            met: staging.effective_level(req_id, tree) >= level,
        })
        .collect()
}

/// Resolves `skill_id` against the tree/catalog/staging and builds its view.
/// `None` when the skill is not in the tree — nothing to show without server
/// state (level, max level, requires) for it.
pub fn build_skill_view(
    skill_id: u32,
    catalog: Option<&SkillCatalog>,
    tree: &SkillTreeState,
    staging: &SkillPanelStaging,
    status: Option<&CharacterStatus>,
) -> Option<SkillInfoView> {
    let node = tree.skills.get(&skill_id)?;
    let effective = staging.effective_level(skill_id, tree);
    let requires = requires_for(node, tree, staging, catalog);
    let requires_met = requires.iter().all(|chip| chip.met);
    let edge = skill_edge_grade(effective, node.max_level, requires_met);
    let unlocks = unlocks_for(skill_id, tree, staging, catalog);

    let meta = catalog.and_then(|c| c.get(skill_id));
    let sp_cost = meta
        .and_then(|m| level_value(&m.sp_cost, effective))
        .map(|v| v.to_string());
    let range = meta
        .and_then(|m| level_value(&m.attack_range, effective))
        .map(|v| v.to_string());
    let description = meta
        .map(|m| super::description::parse(&m.description, &m.display_name))
        .unwrap_or_default();

    let (can_raise, points_left) = match status {
        Some(status) => (
            staging.can_raise(skill_id, tree, status, status.skill_point),
            staging.points_left(status.skill_point),
        ),
        None => (false, 0),
    };

    Some(SkillInfoView {
        icon_path: catalog.and_then(|c| c.icon_path(skill_id)),
        edge,
        name: skill_name(skill_id, catalog),
        kind: kind_label(node.inf_type),
        level_line: format!("{effective}/{}", node.max_level),
        description,
        sp_cost,
        range,
        requires,
        unlocks,
        can_raise,
        points_left,
    })
}

/// The info-only view for a guild skill. Guild skills carry no prerequisite tree,
/// so requires/unlocks stay empty and raising is not offered here.
pub fn guild_skill_view(skill: &GuildSkillInfo, catalog: Option<&SkillCatalog>) -> SkillInfoView {
    let skill_id = skill.skill_id;
    let meta = catalog.and_then(|c| c.get(skill_id));
    let description = meta
        .map(|m| super::description::parse(&m.description, &m.display_name))
        .unwrap_or_default();
    SkillInfoView {
        icon_path: catalog.and_then(|c| c.icon_path(skill_id)),
        edge: skill_edge_grade(skill.level, skill.max_level, true),
        name: skill_name(skill_id, catalog),
        kind: "Guild".to_string(),
        level_line: format!("{}/{}", skill.level, skill.max_level),
        description,
        sp_cost: meta
            .and_then(|m| level_value(&m.sp_cost, skill.level))
            .map(|v| v.to_string()),
        range: meta
            .and_then(|m| level_value(&m.attack_range, skill.level))
            .map(|v| v.to_string()),
        requires: Vec::new(),
        unlocks: Vec::new(),
        can_raise: false,
        points_left: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_client::domain::inventory::Item;
    use lifthrasir_client::domain::skill::SkillNode;
    use lifthrasir_data::{ItemData, ItemInfo, SkillData, SkillMeta};
    use net_contract::dto::{CartItem, StorageItem};
    use std::collections::HashMap;

    // -----------------------------------------------------------------
    // Item fixtures.
    // -----------------------------------------------------------------

    fn item_db() -> ItemDb {
        let mut data = ItemData::default();
        data.items.insert(
            501,
            ItemInfo {
                identified_name: "Red Potion".to_string(),
                identified_resource: "RED_POTION".to_string(),
                identified_description: vec!["Restores 45 HP.".to_string()],
                unidentified_name: "Unknown Potion".to_string(),
                unidentified_resource: "UNKNOWN_POTION".to_string(),
                unidentified_description: vec!["An unidentified potion.".to_string()],
                slot_count: 0,
            },
        );
        data.items.insert(
            2104,
            ItemInfo {
                identified_name: "Buckler".to_string(),
                identified_resource: "BUCKLER".to_string(),
                identified_description: vec!["A small round ^ff0000shield^000000.".to_string()],
                unidentified_name: "Round Shield".to_string(),
                unidentified_resource: "ROUND_SHIELD".to_string(),
                unidentified_description: vec!["An unidentified shield.".to_string()],
                slot_count: 1,
            },
        );
        data.items.insert(
            4001,
            ItemInfo {
                identified_name: "Poring Card".to_string(),
                identified_resource: "PORING_CARD".to_string(),
                identified_description: vec![],
                unidentified_name: "Poring Card".to_string(),
                unidentified_resource: "PORING_CARD".to_string(),
                unidentified_description: vec![],
                slot_count: 0,
            },
        );
        ItemDb::from_item_data(data)
    }

    fn option_db() -> RandomOptionDb {
        let mut data = lifthrasir_data::RandomOptionData::default();
        data.names.insert(3, "STR + %d".to_string());
        data.names.insert(9, "MHP + %d%%".to_string());
        RandomOptionDb::from_random_option_data(data)
    }

    fn option(id: i32, value: i32) -> lifthrasir_client::domain::inventory::ItemOption {
        lifthrasir_client::domain::inventory::ItemOption {
            id,
            value,
            param: 0,
        }
    }

    fn wire_option(id: i32, value: i32) -> net_contract::dto::ItemOption {
        net_contract::dto::ItemOption {
            id,
            value,
            param: 0,
        }
    }

    fn equip_item(index: u16, refine: u8, cards: [u32; 4], identified: bool) -> Item {
        Item {
            index,
            item_id: 2104,
            item_type: 5,
            amount: 1,
            location: 0x0020,
            wear_state: 0,
            refine,
            cards,
            identified,
            ..Default::default()
        }
    }

    fn storage_item(index: u32, weight: u32) -> StorageItem {
        StorageItem {
            index,
            nameid: 501,
            amount: 1,
            type_: 0,
            location: 0,
            attribute: 0,
            refine: 0,
            expire_time: 0,
            look: 0,
            weight,
            identified: true,
            cards: vec![],
            options: vec![],
        }
    }

    fn cart_item(index: u32, weight: u32) -> CartItem {
        CartItem {
            nameid: 501,
            index,
            amount: 1,
            identified: true,
            refine: 0,
            cards: vec![],
            options: vec![],
            attribute: 0,
            expire_time: 0,
            weight,
        }
    }

    #[test]
    fn item_edge_grade_boundaries() {
        assert_eq!(item_edge_grade(3, false), EdgeGrade::Common);
        assert_eq!(item_edge_grade(4, false), EdgeGrade::Fine);
        assert_eq!(item_edge_grade(6, false), EdgeGrade::Fine);
        assert_eq!(item_edge_grade(7, false), EdgeGrade::Rare);
        assert_eq!(item_edge_grade(0, true), EdgeGrade::Fine);
    }

    #[test]
    fn identified_equip_grades_by_refine_and_cards() {
        let view = inventory_item_view(&equip_item(2, 7, [4001, 0, 0, 0], true), &item_db(), None);

        assert_eq!(view.edge, EdgeGrade::Rare);
        assert_eq!(view.refine, Some(7));
        assert_eq!(view.cards, vec!["Poring Card".to_string()]);
        assert_eq!(view.sockets_filled, 1);
        assert_eq!(view.sockets_total, 1);
    }

    #[test]
    fn unidentified_item_suppresses_refine_cards_and_pips() {
        let view = inventory_item_view(&equip_item(2, 7, [4001, 0, 0, 0], false), &item_db(), None);

        assert!(!view.identified);
        assert_eq!(view.name, "Round Shield");
        assert_eq!(view.refine, None);
        assert!(view.cards.is_empty());
        assert_eq!(view.sockets_filled, 0);
        assert_eq!(view.sockets_total, 0);
        assert_eq!(view.edge, EdgeGrade::Common);
    }

    #[test]
    fn unknown_item_id_falls_back_to_placeholder_name() {
        let item = Item {
            index: 2,
            item_id: 9999,
            identified: true,
            ..Default::default()
        };

        let view = inventory_item_view(&item, &item_db(), None);

        assert_eq!(view.name, "Item #9999");
        assert!(view.description.is_empty());
        assert!(view.icon_path.is_none());
        assert!(view.illustration_path.is_none());
    }

    #[test]
    fn inventory_item_carries_no_meta() {
        let view = inventory_item_view(&equip_item(2, 0, [0; 4], true), &item_db(), None);

        assert!(view.meta.is_empty());
    }

    #[test]
    fn random_options_render_in_slot_order() {
        let mut item = equip_item(2, 0, [0; 4], true);
        item.options = vec![option(3, 5), option(0, 0), option(9, 4)];

        let view = inventory_item_view(&item, &item_db(), Some(&option_db()));

        assert_eq!(
            view.options,
            vec!["STR + 5".to_string(), "MHP + 4%".to_string()]
        );
    }

    #[test]
    fn unknown_option_id_falls_back_to_raw_id_and_value() {
        let mut item = equip_item(2, 0, [0; 4], true);
        item.options = vec![option(9999, 7)];

        let view = inventory_item_view(&item, &item_db(), Some(&option_db()));

        assert_eq!(view.options, vec!["Option #9999: 7".to_string()]);
    }

    #[test]
    fn missing_option_catalog_still_shows_options() {
        let mut item = equip_item(2, 0, [0; 4], true);
        item.options = vec![option(3, 5)];

        let view = inventory_item_view(&item, &item_db(), None);

        assert_eq!(view.options, vec!["Option #3: 5".to_string()]);
    }

    #[test]
    fn unidentified_item_suppresses_random_options() {
        let mut item = equip_item(2, 0, [0; 4], false);
        item.options = vec![option(3, 5)];

        let view = inventory_item_view(&item, &item_db(), Some(&option_db()));

        assert!(view.options.is_empty());
    }

    #[test]
    fn storage_and_cart_items_carry_random_options() {
        let mut stored = storage_item(3, 25);
        stored.options = vec![wire_option(3, 5)];
        let mut carted = cart_item(1, 30);
        carted.options = vec![wire_option(9, 4)];

        let stored_view = storage_item_view(&stored, &item_db(), Some(&option_db()));
        let cart_view = cart_item_view(&carted, &item_db(), Some(&option_db()));

        assert_eq!(stored_view.options, vec!["STR + 5".to_string()]);
        assert_eq!(cart_view.options, vec!["MHP + 4%".to_string()]);
    }

    #[test]
    fn storage_item_carries_weight_meta() {
        let view = storage_item_view(&storage_item(3, 25), &item_db(), None);

        assert_eq!(view.meta, vec![("Weight".to_string(), "25".to_string())]);
        assert_eq!(view.favorite, None);
    }

    #[test]
    fn cart_item_carries_weight_meta() {
        let view = cart_item_view(&cart_item(1, 30), &item_db(), None);

        assert_eq!(view.meta, vec![("Weight".to_string(), "30".to_string())]);
        assert_eq!(view.favorite, None);
    }

    #[test]
    fn shop_item_carries_price_meta() {
        let view = shop_item_view(501, Some(500), &item_db());

        assert_eq!(view.meta, vec![("Price".to_string(), "500z".to_string())]);
        assert!(view.identified);
        assert_eq!(view.favorite, None);
    }

    #[test]
    fn shop_item_with_no_price_has_no_meta_but_still_resolves() {
        let view = shop_item_view(501, None, &item_db());

        assert!(view.meta.is_empty());
        assert_eq!(view.name, "Red Potion");
    }

    #[test]
    fn inventory_item_carries_favorite_flag() {
        let mut item = equip_item(2, 0, [0; 4], true);
        item.favorite = true;

        assert_eq!(
            inventory_item_view(&item, &item_db(), None).favorite,
            Some(true)
        );
        item.favorite = false;
        assert_eq!(
            inventory_item_view(&item, &item_db(), None).favorite,
            Some(false)
        );
    }

    // -----------------------------------------------------------------
    // Vending stock fixtures and tests.
    // -----------------------------------------------------------------

    fn vending_listing() -> VendingListing {
        VendingListing {
            index: 3,
            nameid: 2104,
            type_: 5,
            amount: 1,
            identified: true,
            attribute: 0,
            refine: 0,
            cards: vec![],
            options: vec![],
            price: 5_000,
            signer_name: String::new(),
            creator_id: 0,
            creator_kind: VendingCreatorKind::None,
            enchant_grade: 0,
        }
    }

    #[test]
    fn vending_listing_carries_price_and_instance_metadata() {
        let mut item = vending_listing();
        item.refine = 7;
        item.cards = vec![4001, 0];
        item.options = vec![wire_option(3, 5), wire_option(9, -4)];
        item.creator_kind = VendingCreatorKind::Signed;
        item.signer_name = "Odin".into();
        item.creator_id = 42;
        item.enchant_grade = 2;

        let view = vending_item_view(&item, &item_db(), Some(&option_db()));

        assert_eq!(view.name, "Buckler");
        assert_eq!(view.refine, Some(7));
        assert_eq!(view.cards, vec!["Poring Card".to_string()]);
        assert_eq!(
            view.options,
            vec!["STR + 5".to_string(), "MHP + -4%".to_string()]
        );
        assert_eq!(view.meta[0], ("Price".to_string(), "5000z".to_string()));
        assert_eq!(
            view.meta[1],
            ("Signature".to_string(), "Odin (#42)".to_string())
        );
        assert!(view.tags.contains(&"Grade C".to_string()));
        assert_eq!(view.favorite, None);
    }

    #[test]
    fn same_nameid_instances_stay_distinguishable() {
        let plain = vending_listing();
        let mut refined = plain.clone();
        refined.refine = 4;
        let mut carded = plain.clone();
        carded.cards = vec![4001];
        let mut optimised = plain.clone();
        optimised.options = vec![wire_option(3, 5)];
        let mut forged = plain.clone();
        forged.creator_kind = VendingCreatorKind::Forged;
        forged.signer_name = "Loki".into();
        let mut broken = plain.clone();
        broken.attribute = 1;
        let mut graded = plain.clone();
        graded.enchant_grade = 4;

        let views: Vec<ItemInfoView> = [plain, refined, carded, optimised, forged, broken, graded]
            .iter()
            .map(|item| vending_item_view(item, &item_db(), Some(&option_db())))
            .collect();

        for (index, view) in views.iter().enumerate() {
            assert_eq!(view.name, "Buckler", "instance {index} keeps the name");
        }
        assert_eq!(views[0].refine, None);
        assert_eq!(views[1].refine, Some(4));
        assert_eq!(views[2].sockets_filled, 1);
        assert_eq!(views[3].options, vec!["STR + 5".to_string()]);
        assert_eq!(
            views[4].meta,
            vec![
                ("Price".to_string(), "5000z".to_string()),
                ("Forged by".to_string(), "Loki".to_string()),
            ]
        );
        assert!(views[5].tags.contains(&"Broken".to_string()));
        assert!(views[6].tags.contains(&"Grade A".to_string()));
        assert_eq!(views[0].edge, EdgeGrade::Common);
        assert_eq!(views[1].edge, EdgeGrade::Fine);
    }

    #[test]
    fn broken_bit_only_marks_actual_broken_items() {
        let mut item = vending_listing();
        assert!(
            !vending_item_view(&item, &item_db(), None)
                .tags
                .contains(&"Broken".to_string())
        );

        item.attribute = 2;
        assert!(
            !vending_item_view(&item, &item_db(), None)
                .tags
                .contains(&"Broken".to_string())
        );

        item.attribute = 3;
        assert!(
            vending_item_view(&item, &item_db(), None)
                .tags
                .contains(&"Broken".to_string())
        );
    }

    #[test]
    fn enchant_grades_use_verified_names_and_keep_unknown_numbers() {
        let mut item = vending_listing();
        let mut grades = Vec::new();
        for grade in 0..=5 {
            item.enchant_grade = grade;
            let view = vending_item_view(&item, &item_db(), None);
            grades.push(view.tags.into_iter().find(|tag| tag.starts_with("Grade")));
        }
        assert_eq!(
            grades,
            vec![
                None,
                Some("Grade D".to_string()),
                Some("Grade C".to_string()),
                Some("Grade B".to_string()),
                Some("Grade A".to_string()),
                Some("Grade #5".to_string()),
            ]
        );
    }

    #[test]
    fn unknown_creator_kind_and_unsigned_creators_are_visible() {
        let mut item = vending_listing();
        item.creator_kind = VendingCreatorKind::Unknown(9);
        let view = vending_item_view(&item, &item_db(), None);
        assert_eq!(
            view.meta[1],
            ("Creator".to_string(), "Unknown (9)".to_string())
        );

        item.creator_kind = VendingCreatorKind::Signed;
        assert_eq!(
            vending_item_view(&item, &item_db(), None).meta.len(),
            1,
            "a signature without a name adds nothing"
        );

        item.creator_id = 77;
        assert_eq!(
            vending_item_view(&item, &item_db(), None).meta[1],
            ("Signature".to_string(), "#77".to_string())
        );
    }

    #[test]
    fn unidentified_vending_stock_suppresses_instance_details() {
        let mut item = vending_listing();
        item.identified = false;
        item.refine = 7;
        item.cards = vec![4001];
        item.options = vec![wire_option(3, 5)];

        let view = vending_item_view(&item, &item_db(), Some(&option_db()));

        assert!(!view.identified);
        assert_eq!(view.name, "Round Shield");
        assert_eq!(view.refine, None);
        assert!(view.cards.is_empty());
        assert!(view.options.is_empty());
        assert_eq!(view.meta, vec![("Price".to_string(), "5000z".to_string())]);
    }

    #[test]
    fn unknown_vending_stock_keeps_placeholders_and_option_fallback() {
        let mut item = vending_listing();
        item.nameid = 9999;
        item.options = vec![wire_option(9999, 7)];

        let view = vending_item_view(&item, &item_db(), Some(&option_db()));

        assert_eq!(view.name, "Item #9999");
        assert!(view.description.is_empty());
        assert_eq!(view.options, vec!["Option #9999: 7".to_string()]);
        assert_eq!(view.meta, vec![("Price".to_string(), "5000z".to_string())]);
    }

    #[test]
    fn equips_to_tags_decode_from_wear_mask() {
        let view = inventory_item_view(&equip_item(2, 0, [0; 4], true), &item_db(), None);

        assert!(view.tags.contains(&"Shield".to_string()));
    }

    #[test]
    fn description_carries_color_runs() {
        let view = inventory_item_view(&equip_item(2, 0, [0; 4], true), &item_db(), None);

        assert_eq!(
            view.description,
            vec![vec![
                (lifthrasir_ui::theme::TEXT_DIM, "A small round ".to_string()),
                (Color::srgb_u8(0xff, 0, 0), "shield".to_string()),
                (lifthrasir_ui::theme::TEXT_DIM, ".".to_string()),
            ]]
        );
    }

    // -----------------------------------------------------------------
    // Skill fixtures.
    // -----------------------------------------------------------------

    fn skill_catalog() -> SkillCatalog {
        let mut data = SkillData::default();
        data.skills.insert(
            5,
            SkillMeta {
                name: "SM_BASH".to_string(),
                display_name: "Bash".to_string(),
                description: vec!["Deals ^ff0000heavy^000000 damage.".to_string()],
                max_level: 10,
                sp_cost: vec![8, 9, 10],
                attack_range: vec![1, 1, 1],
            },
        );
        data.skills.insert(
            17,
            SkillMeta {
                name: "SM_MAGNUM".to_string(),
                display_name: "Magnum Break".to_string(),
                description: vec![],
                max_level: 5,
                sp_cost: vec![15],
                attack_range: vec![],
            },
        );
        data.skills.insert(
            9,
            SkillMeta {
                name: "SM_ENDURE".to_string(),
                display_name: "Endure".to_string(),
                description: vec![],
                max_level: 1,
                sp_cost: vec![],
                attack_range: vec![],
            },
        );
        SkillCatalog::from_skill_data(data)
    }

    fn node(level: u32, max_level: u32, requires: Vec<(u32, u32)>, inf_type: u32) -> SkillNode {
        SkillNode {
            level,
            max_level,
            upgradable: true,
            requires,
            req_base_level: 0,
            req_job_level: 0,
            sp: 8,
            range: 1,
            inf_type,
            job_id: 1,
            splash_radius: 0,
        }
    }

    fn tree() -> SkillTreeState {
        let mut skills = HashMap::new();
        skills.insert(9, node(1, 1, vec![], 0));
        skills.insert(5, node(3, 10, vec![(9, 1)], 1));
        skills.insert(17, node(0, 5, vec![(5, 5)], 2));
        SkillTreeState { skills }
    }

    fn status(base_level: u32, job_level: u32, skill_point: u32) -> CharacterStatus {
        CharacterStatus {
            base_level,
            job_level,
            skill_point,
            ..Default::default()
        }
    }

    #[test]
    fn maxed_skill_grades_rare() {
        assert_eq!(skill_edge_grade(10, 10, true), EdgeGrade::Rare);
    }

    #[test]
    fn unmet_requires_grades_common() {
        assert_eq!(skill_edge_grade(0, 10, false), EdgeGrade::Common);
    }

    #[test]
    fn learnable_skill_grades_fine() {
        assert_eq!(skill_edge_grade(3, 10, true), EdgeGrade::Fine);
    }

    #[test]
    fn requires_chips_carry_met_state() {
        let catalog = skill_catalog();
        let t = tree();
        let staging = SkillPanelStaging::default();

        let view = build_skill_view(5, Some(&catalog), &t, &staging, None).unwrap();

        assert_eq!(view.requires.len(), 1);
        assert_eq!(view.requires[0].skill_id, 9);
        assert_eq!(view.requires[0].name, "Endure");
        assert!(view.requires[0].met);
        assert_eq!(view.edge, EdgeGrade::Fine);
    }

    #[test]
    fn skill_with_unmet_requires_grades_common_end_to_end() {
        let catalog = skill_catalog();
        let mut t = tree();
        t.skills.get_mut(&9).unwrap().level = 0;
        let staging = SkillPanelStaging::default();

        let view = build_skill_view(5, Some(&catalog), &t, &staging, None).unwrap();

        assert!(!view.requires[0].met);
        assert_eq!(view.edge, EdgeGrade::Common);
    }

    #[test]
    fn unlocks_reverse_index_finds_dependents() {
        let catalog = skill_catalog();
        let t = tree();
        let staging = SkillPanelStaging::default();

        let view = build_skill_view(9, Some(&catalog), &t, &staging, None).unwrap();

        assert_eq!(view.unlocks.len(), 1);
        assert_eq!(view.unlocks[0].skill_id, 5);
        assert_eq!(view.unlocks[0].name, "Bash");
    }

    #[test]
    fn skill_with_no_dependents_has_empty_unlocks() {
        let catalog = skill_catalog();
        let t = tree();
        let staging = SkillPanelStaging::default();

        let view = build_skill_view(17, Some(&catalog), &t, &staging, None).unwrap();

        assert!(view.unlocks.is_empty());
    }

    #[test]
    fn unknown_skill_id_yields_no_view() {
        let catalog = skill_catalog();
        let t = tree();
        let staging = SkillPanelStaging::default();

        let view = build_skill_view(9999, Some(&catalog), &t, &staging, None);

        assert!(view.is_none());
    }

    #[test]
    fn sp_and_range_read_from_effective_level_index() {
        let catalog = skill_catalog();
        let t = tree();
        let staging = SkillPanelStaging::default();

        let view = build_skill_view(5, Some(&catalog), &t, &staging, None).unwrap();

        assert_eq!(view.sp_cost, Some("10".to_string()));
        assert_eq!(view.range, Some("1".to_string()));
    }

    #[test]
    fn can_raise_true_when_requires_met_and_points_available() {
        let catalog = skill_catalog();
        let t = tree();
        let staging = SkillPanelStaging::default();
        let s = status(10, 10, 3);

        let view = build_skill_view(5, Some(&catalog), &t, &staging, Some(&s)).unwrap();

        assert!(view.can_raise);
        assert_eq!(view.points_left, 3);
    }

    #[test]
    fn can_raise_false_when_no_points_left() {
        let catalog = skill_catalog();
        let t = tree();
        let staging = SkillPanelStaging::default();
        let s = status(10, 10, 0);

        let view = build_skill_view(5, Some(&catalog), &t, &staging, Some(&s)).unwrap();

        assert!(!view.can_raise);
        assert_eq!(view.points_left, 0);
    }

    #[test]
    fn false_upgradable_view_is_enabled_only_after_staging_missing_prerequisites() {
        let catalog = skill_catalog();
        let mut t = tree();
        t.skills.get_mut(&17).unwrap().upgradable = false;
        let s = status(10, 10, 3);
        let mut staging = SkillPanelStaging::default();

        assert!(
            !build_skill_view(17, Some(&catalog), &t, &staging, Some(&s))
                .unwrap()
                .can_raise
        );
        staging.raise(5, &t, &s, s.skill_point);
        staging.raise(5, &t, &s, s.skill_point);
        assert!(
            build_skill_view(17, Some(&catalog), &t, &staging, Some(&s))
                .unwrap()
                .can_raise
        );
    }

    #[test]
    fn unexplained_false_upgradable_view_stays_disabled() {
        let catalog = skill_catalog();
        let mut t = tree();
        t.skills.get_mut(&5).unwrap().upgradable = false;
        let staging = SkillPanelStaging::default();
        let s = status(10, 10, 3);

        assert!(
            !build_skill_view(5, Some(&catalog), &t, &staging, Some(&s))
                .unwrap()
                .can_raise
        );
    }

    #[test]
    fn can_raise_false_without_a_status() {
        let catalog = skill_catalog();
        let t = tree();
        let staging = SkillPanelStaging::default();

        let view = build_skill_view(5, Some(&catalog), &t, &staging, None).unwrap();

        assert!(!view.can_raise);
        assert_eq!(view.points_left, 0);
    }

    #[test]
    fn kind_tag_reflects_inf_type() {
        let catalog = skill_catalog();
        let t = tree();
        let staging = SkillPanelStaging::default();

        let passive = build_skill_view(9, Some(&catalog), &t, &staging, None).unwrap();
        let active = build_skill_view(5, Some(&catalog), &t, &staging, None).unwrap();

        assert_eq!(passive.kind, "Passive");
        assert_eq!(active.kind, "Active");
    }

    #[test]
    fn guild_skill_view_reads_level_from_the_snapshot() {
        let skill = GuildSkillInfo {
            skill_id: 10_000,
            level: 2,
            max_level: 5,
        };

        let view = guild_skill_view(&skill, None);

        assert_eq!(view.name, "#10000");
        assert_eq!(view.kind, "Guild");
        assert_eq!(view.level_line, "2/5");
        assert_eq!(view.edge, EdgeGrade::Fine);
        assert!(view.requires.is_empty());
        assert!(!view.can_raise);
    }
}
