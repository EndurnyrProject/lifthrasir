//! View-model builders for the info modal: [`build_item_view`] and
//! [`build_skill_view`] resolve an [`ItemRef`]/skill id plus the live domain
//! resources into plain [`ItemInfoView`]/[`SkillInfoView`] structs. Every optional
//! section is `Option`/`Vec` so the scenes (item_scene.rs/skill_scene.rs, later
//! tasks) can skip absent sections without special-casing.
//!
//! These builders are pure functions over borrowed resource references — no
//! `Res<T>`/ECS access — so they are unit-testable without spinning up an `App`.

use bevy::prelude::Color;

use game_engine::domain::assets::item_icon_path;
use game_engine::domain::cart::Cart;
use game_engine::domain::entities::character::components::equipment::EquipmentSlot;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::equipment::decode_wear_location;
use game_engine::domain::inventory::item::item_category;
use game_engine::domain::inventory::{Inventory, ItemCategory};
use game_engine::domain::skill::{SkillNode, SkillTreeState};
use game_engine::domain::storage::Storage;
use game_engine::infrastructure::item::ItemDb;
use game_engine::infrastructure::skill::SkillCatalog;

use crate::rich_text::parse_color_codes;
use crate::theme;
use crate::widgets::character_window::SkillPanelStaging;
use crate::widgets::shop_window::ShopSession;
use crate::widgets::storage_window::StorageSelection;

use super::shell::EdgeGrade;
use super::ItemRef;

/// One description line split into `^RRGGBB`-colored runs (already parsed —
/// scenes render each run as-is, no further `parse_color_codes` call needed).
pub type ColoredLine = Vec<(Color, String)>;

/// The item modal's content, section by section. `None`/empty fields mean the
/// section is absent for this item/context.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemInfoView {
    /// The resolved item's `item_id`, carried through so the footer can
    /// revalidate the item at its stored index is still the one shown.
    pub item_id: u32,
    pub icon_path: Option<String>,
    pub edge: EdgeGrade,
    pub name: String,
    pub identified: bool,
    pub tags: Vec<String>,
    pub refine: Option<i32>,
    pub sockets_filled: u8,
    pub sockets_total: u8,
    pub cards: Vec<String>,
    pub description: Vec<ColoredLine>,
    /// Contextual `(label, value)` meta-grid rows — weight for Storage/Cart refs,
    /// price for ShopBuy, empty otherwise.
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

/// An item instance resolved from its `ItemRef`, stripped down to the fields the
/// view needs, regardless of which domain payload it came from.
struct ResolvedItem<'a> {
    item_id: u32,
    identified: bool,
    refine: u8,
    cards: &'a [u32],
    wear_mask: u32,
    category_label: Option<&'static str>,
    weight: Option<u32>,
    price: Option<u32>,
}

/// Resolves `item_ref` against the live resources into a [`ResolvedItem`]. `None`
/// when the referenced slot/selection is empty (e.g. the item was consumed or
/// moved between the right-click and the build).
fn resolve_item<'a>(
    item_ref: ItemRef,
    inventory: &'a Inventory,
    storage: &'a Storage,
    cart: &'a Cart,
    shop: Option<&'a ShopSession>,
) -> Option<ResolvedItem<'a>> {
    match item_ref {
        ItemRef::Inventory(index)
        | ItemRef::Equipped(index)
        | ItemRef::Storage(StorageSelection::Bag(index)) => {
            let item = inventory.get(index)?;
            Some(ResolvedItem {
                item_id: item.item_id,
                identified: item.identified,
                refine: item.refine,
                cards: &item.cards,
                wear_mask: item.location,
                category_label: Some(category_label(item.category())),
                weight: None,
                price: None,
            })
        }
        ItemRef::Storage(StorageSelection::Vault(index)) => {
            let item = storage.get(index)?;
            Some(ResolvedItem {
                item_id: item.nameid,
                identified: item.identified,
                refine: item.refine as u8,
                cards: &item.cards,
                wear_mask: item.location,
                category_label: Some(category_label(item_category(item.type_))),
                weight: Some(item.weight),
                price: None,
            })
        }
        ItemRef::Cart(index) => {
            let item = cart.get(index)?;
            Some(ResolvedItem {
                item_id: item.nameid,
                identified: item.identified,
                refine: item.refine as u8,
                cards: &item.cards,
                wear_mask: 0,
                category_label: None,
                weight: Some(item.weight),
                price: None,
            })
        }
        ItemRef::ShopBuy(nameid) => {
            let price = shop
                .and_then(|s| s.buy_items.iter().find(|i| i.nameid == nameid))
                .map(|i| i.price);
            Some(ResolvedItem {
                item_id: nameid,
                identified: true,
                refine: 0,
                cards: &[],
                wear_mask: 0,
                category_label: None,
                weight: None,
                price,
            })
        }
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

/// Resolves `item_ref` and builds its view. `None` when the referenced
/// slot/selection no longer holds an item. An `item_id` absent from `item_db`
/// still yields a view — `Item #<id>`, no description — rather than panicking.
pub fn build_item_view(
    item_ref: ItemRef,
    item_db: &ItemDb,
    inventory: &Inventory,
    storage: &Storage,
    cart: &Cart,
    shop: Option<&ShopSession>,
) -> Option<ItemInfoView> {
    let resolved = resolve_item(item_ref, inventory, storage, cart, shop)?;
    Some(item_view_from_resolved(resolved, item_db))
}

fn item_view_from_resolved(resolved: ResolvedItem, item_db: &ItemDb) -> ItemInfoView {
    let item_id = resolved.item_id;
    let identified = resolved.identified;
    let name = item_db
        .name(resolved.item_id, identified)
        .map(str::to_string)
        .unwrap_or_else(|| format!("Item #{}", resolved.item_id));
    let description = item_db
        .description(resolved.item_id, identified)
        .map(|lines| {
            lines
                .iter()
                .map(|line| parse_color_codes(line, theme::TEXT_DIM))
                .collect()
        })
        .unwrap_or_default();
    let icon_path = item_db
        .icon_resource(resolved.item_id, identified)
        .map(item_icon_path);

    let (refine, sockets_filled, sockets_total, cards, edge) = if identified {
        let slot_count = item_db.slot_count(resolved.item_id).unwrap_or(0);
        let cards = card_names(resolved.cards, slot_count, item_db);
        let refine = (resolved.refine > 0).then_some(resolved.refine as i32);
        let edge = item_edge_grade(resolved.refine, !cards.is_empty());
        (refine, cards.len() as u8, slot_count, cards, edge)
    } else {
        (None, 0, 0, Vec::new(), EdgeGrade::Common)
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
        edge,
        name,
        identified,
        tags,
        refine,
        sockets_filled,
        sockets_total,
        cards,
        description,
        meta,
    }
}

// ---------------------------------------------------------------------------
// Skill view builder.
// ---------------------------------------------------------------------------

fn skill_name(id: u32, catalog: Option<&SkillCatalog>) -> String {
    catalog
        .and_then(|c| c.get(id))
        .map(|m| m.display_name.clone())
        .unwrap_or_else(|| format!("#{id}"))
}

fn kind_label(inf: u32) -> String {
    use game_engine::domain::skill::{form, Form};
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
        .map(|m| {
            m.description
                .iter()
                .map(|line| parse_color_codes(line, theme::TEXT_DIM))
                .collect()
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::domain::inventory::Item;
    use game_engine::domain::skill::SkillNode;
    use lifthrasir_data::{ItemData, ItemInfo, SkillData, SkillMeta};
    use net_contract::dto::{CartItem, ShopBuyItem, StorageItem};
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
            attribute: 0,
            expire_time: 0,
            weight,
        }
    }

    fn shop_session() -> ShopSession {
        ShopSession {
            unit_id: 1,
            buy_items: vec![ShopBuyItem {
                nameid: 501,
                price: 500,
            }],
            sell_items: vec![],
            tab: Default::default(),
            cart_buy: HashMap::new(),
            cart_sell: HashMap::new(),
            selected: None,
            pending_qty: 1,
            banner: None,
            confirm_open: false,
            awaiting: false,
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
        let db = item_db();
        let inventory = {
            let mut inv = Inventory::default();
            inv.upsert(equip_item(2, 7, [4001, 0, 0, 0], true));
            inv
        };
        let storage = Storage::default();
        let cart = Cart::default();

        let view = build_item_view(
            ItemRef::Inventory(2),
            &db,
            &inventory,
            &storage,
            &cart,
            None,
        )
        .unwrap();

        assert_eq!(view.edge, EdgeGrade::Rare);
        assert_eq!(view.refine, Some(7));
        assert_eq!(view.cards, vec!["Poring Card".to_string()]);
        assert_eq!(view.sockets_filled, 1);
        assert_eq!(view.sockets_total, 1);
    }

    #[test]
    fn unidentified_item_suppresses_refine_cards_and_pips() {
        let db = item_db();
        let inventory = {
            let mut inv = Inventory::default();
            inv.upsert(equip_item(2, 7, [4001, 0, 0, 0], false));
            inv
        };
        let storage = Storage::default();
        let cart = Cart::default();

        let view = build_item_view(
            ItemRef::Inventory(2),
            &db,
            &inventory,
            &storage,
            &cart,
            None,
        )
        .unwrap();

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
        let db = item_db();
        let inventory = {
            let mut inv = Inventory::default();
            inv.upsert(Item {
                index: 2,
                item_id: 9999,
                identified: true,
                ..Default::default()
            });
            inv
        };
        let storage = Storage::default();
        let cart = Cart::default();

        let view = build_item_view(
            ItemRef::Inventory(2),
            &db,
            &inventory,
            &storage,
            &cart,
            None,
        )
        .unwrap();

        assert_eq!(view.name, "Item #9999");
        assert!(view.description.is_empty());
        assert!(view.icon_path.is_none());
    }

    #[test]
    fn missing_inventory_slot_yields_no_view() {
        let db = item_db();
        let inventory = Inventory::default();
        let storage = Storage::default();
        let cart = Cart::default();

        let view = build_item_view(
            ItemRef::Inventory(2),
            &db,
            &inventory,
            &storage,
            &cart,
            None,
        );

        assert!(view.is_none());
    }

    #[test]
    fn inventory_and_equipped_refs_carry_no_meta() {
        let db = item_db();
        let inventory = {
            let mut inv = Inventory::default();
            inv.upsert(equip_item(2, 0, [0; 4], true));
            inv
        };
        let storage = Storage::default();
        let cart = Cart::default();

        let inventory_view = build_item_view(
            ItemRef::Inventory(2),
            &db,
            &inventory,
            &storage,
            &cart,
            None,
        )
        .unwrap();
        let equipped_view =
            build_item_view(ItemRef::Equipped(2), &db, &inventory, &storage, &cart, None).unwrap();

        assert!(inventory_view.meta.is_empty());
        assert!(equipped_view.meta.is_empty());
    }

    #[test]
    fn storage_bag_selection_carries_no_meta() {
        let db = item_db();
        let inventory = {
            let mut inv = Inventory::default();
            inv.upsert(equip_item(2, 0, [0; 4], true));
            inv
        };
        let storage = Storage::default();
        let cart = Cart::default();

        let view = build_item_view(
            ItemRef::Storage(StorageSelection::Bag(2)),
            &db,
            &inventory,
            &storage,
            &cart,
            None,
        )
        .unwrap();

        assert!(view.meta.is_empty());
    }

    #[test]
    fn storage_vault_selection_carries_weight_meta() {
        let db = item_db();
        let inventory = Inventory::default();
        let storage = {
            let mut s = Storage::default();
            s.open(100, vec![storage_item(3, 25)]);
            s
        };
        let cart = Cart::default();

        let view = build_item_view(
            ItemRef::Storage(StorageSelection::Vault(3)),
            &db,
            &inventory,
            &storage,
            &cart,
            None,
        )
        .unwrap();

        assert_eq!(view.meta, vec![("Weight".to_string(), "25".to_string())]);
    }

    #[test]
    fn cart_ref_carries_weight_meta() {
        let db = item_db();
        let inventory = Inventory::default();
        let storage = Storage::default();
        let mut cart = Cart::default();
        cart.upsert(cart_item(1, 30));

        let view =
            build_item_view(ItemRef::Cart(1), &db, &inventory, &storage, &cart, None).unwrap();

        assert_eq!(view.meta, vec![("Weight".to_string(), "30".to_string())]);
    }

    #[test]
    fn shop_buy_ref_carries_price_meta() {
        let db = item_db();
        let inventory = Inventory::default();
        let storage = Storage::default();
        let cart = Cart::default();
        let shop = shop_session();

        let view = build_item_view(
            ItemRef::ShopBuy(501),
            &db,
            &inventory,
            &storage,
            &cart,
            Some(&shop),
        )
        .unwrap();

        assert_eq!(view.meta, vec![("Price".to_string(), "500z".to_string())]);
        assert!(view.identified);
    }

    #[test]
    fn shop_buy_ref_with_no_session_has_no_price_meta_but_still_resolves() {
        let db = item_db();
        let inventory = Inventory::default();
        let storage = Storage::default();
        let cart = Cart::default();

        let view = build_item_view(
            ItemRef::ShopBuy(501),
            &db,
            &inventory,
            &storage,
            &cart,
            None,
        )
        .unwrap();

        assert!(view.meta.is_empty());
        assert_eq!(view.name, "Red Potion");
    }

    #[test]
    fn equips_to_tags_decode_from_wear_mask() {
        let db = item_db();
        let inventory = {
            let mut inv = Inventory::default();
            inv.upsert(equip_item(2, 0, [0; 4], true));
            inv
        };
        let storage = Storage::default();
        let cart = Cart::default();

        let view = build_item_view(
            ItemRef::Inventory(2),
            &db,
            &inventory,
            &storage,
            &cart,
            None,
        )
        .unwrap();

        assert!(view.tags.contains(&"Shield".to_string()));
    }

    #[test]
    fn description_carries_color_runs() {
        let db = item_db();
        let inventory = {
            let mut inv = Inventory::default();
            inv.upsert(equip_item(2, 0, [0; 4], true));
            inv
        };
        let storage = Storage::default();
        let cart = Cart::default();

        let view = build_item_view(
            ItemRef::Inventory(2),
            &db,
            &inventory,
            &storage,
            &cart,
            None,
        )
        .unwrap();

        assert_eq!(
            view.description,
            vec![vec![
                (theme::TEXT_DIM, "A small round ".to_string()),
                (Color::srgb_u8(0xff, 0, 0), "shield".to_string()),
                (theme::TEXT_DIM, ".".to_string()),
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
}
