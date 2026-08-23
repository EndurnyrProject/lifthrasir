//! Skill selection menu: the client half of the server's generic
//! `SkillMenu`/`SkillMenuReply` exchange (`SA_AUTOSPELL`, `SA_CREATECON`,
//! `MC_IDENTIFY`, `BS_REPAIRWEAPON`, and the forge/pharmacy production skills).
//!
//! The server parks exactly one pending offer per session, so this window is a
//! single instance driven by an [`ActiveSkillMenu`] resource: an offer inserts
//! it, any change (a catalyst picked or cleared) rebuilds the window from it,
//! and answering or cancelling removes it. Clicking an entry row is the answer —
//! there is no separate confirm step — so catalysts are chosen first, above the
//! entry list.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_feathers::FeathersCorePlugin;
use bevy_feathers::FeathersPlugins;
use game_engine::core::state::GameState;
use game_engine::domain::inventory::Inventory;
use game_engine::infrastructure::item::ItemDb;
use game_engine::infrastructure::skill::SkillCatalog;
use net_contract::commands::AnswerSkillMenu;
use net_contract::events::{SkillMenuKind, SkillMenuOffered};

use crate::theme::feathers_theme::install_norse_theme;

pub mod scene;

/// The server resolves at most three catalysts per forge.
const MAX_CATALYSTS: usize = 3;

/// Star Crumb, then the four elemental stones (fire, water, wind, earth) — the
/// only ids `Production.Catalysts` resolves. Ordered as the chips are rendered.
const CATALYST_IDS: [u32; 5] = [1000, 994, 995, 996, 997];

/// Weapon forging (`BS_DAGGER` … `BS_KNUCKLE`). Only these skills spend
/// catalysts on their success roll, so only they show the catalyst chips —
/// offering them elsewhere would consume star crumbs for nothing.
const FORGE_SKILL_IDS: std::ops::RangeInclusive<u32> = 98..=104;

/// Window-root marker; a single instance exists while a menu is pending.
#[derive(Component, Default, Clone)]
pub struct SkillMenuRoot;

/// The pending offer plus the catalysts picked so far. Present only while the
/// menu is open; every mutation rebuilds the window.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct ActiveSkillMenu {
    pub src_skill_id: u32,
    pub kind: SkillMenuKind,
    pub entry_ids: Vec<u32>,
    pub catalysts: Vec<u32>,
}

/// What a row does when clicked.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkillMenuAction {
    /// Answer with `selected_id: 0`.
    #[default]
    Cancel,
    /// Answer with this entry id and the picked catalysts.
    Select(u32),
    /// Add one of this catalyst item id to the selection.
    AddCatalyst(u32),
    /// Drop every picked catalyst.
    ClearCatalysts,
}

pub struct SkillMenuPlugin;

impl Plugin for SkillMenuPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.add_systems(
            Update,
            (open_menu, sync_window)
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            Update,
            cancel_on_escape
                .run_if(in_state(GameState::InGame).and_then(resource_exists::<ActiveSkillMenu>)),
        );
        app.add_systems(OnExit(GameState::InGame), |mut commands: Commands| {
            commands.remove_resource::<ActiveSkillMenu>()
        });
    }
}

/// A new offer replaces whatever was pending, mirroring the server's single
/// parked menu. The catalyst selection starts empty on every offer.
fn open_menu(mut offers: MessageReader<SkillMenuOffered>, mut commands: Commands) {
    let Some(offer) = offers.read().last() else {
        return;
    };
    commands.insert_resource(ActiveSkillMenu {
        src_skill_id: offer.src_skill_id,
        kind: offer.kind,
        entry_ids: offer.entry_ids.clone(),
        catalysts: Vec::new(),
    });
}

/// Keeps the window in sync with [`ActiveSkillMenu`]: rebuilt whenever the offer
/// or the catalyst selection changes, despawned once the resource is gone
/// (answered, cancelled, or the zone was left). The window is small and only
/// changes on a click, so it is respawned wholesale rather than diffed.
fn sync_window(
    menu: Option<Res<ActiveSkillMenu>>,
    roots: Query<Entity, With<SkillMenuRoot>>,
    inventory: Res<Inventory>,
    item_db: Option<Res<ItemDb>>,
    skills: Option<Res<SkillCatalog>>,
    mut commands: Commands,
) {
    let Some(menu) = menu else {
        for root in &roots {
            commands.entity(root).despawn();
        }
        return;
    };
    if !menu.is_changed() {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }

    let item_db = item_db.as_deref();
    commands
        .spawn_scene(scene::window(
            menu_title(menu.src_skill_id, skills.as_deref()),
            catalyst_summary(&menu.catalysts, item_db),
            catalyst_buttons(&menu, &inventory, item_db),
            entry_rows(&menu, &inventory, item_db, skills.as_deref()),
        ))
        .insert(DespawnOnExit(GameState::InGame));
}

/// The skill's display name, or a neutral title when the catalog hasn't loaded
/// or doesn't know the id.
fn menu_title(skill_id: u32, skills: Option<&SkillCatalog>) -> String {
    skills
        .and_then(|catalog| catalog.get(skill_id))
        .map(|meta| meta.display_name.clone())
        .unwrap_or_else(|| "Select".to_string())
}

/// One `(label, action)` row per offered entry, in server order, plus a trailing
/// `Cancel` row. An empty offer still renders `Cancel` so the pending menu can
/// always be dismissed.
fn entry_rows(
    menu: &ActiveSkillMenu,
    inventory: &Inventory,
    item_db: Option<&ItemDb>,
    skills: Option<&SkillCatalog>,
) -> Vec<(String, SkillMenuAction)> {
    let mut rows: Vec<_> = menu
        .entry_ids
        .iter()
        .map(|&id| {
            (
                entry_label(menu.kind, id, inventory, item_db, skills),
                SkillMenuAction::Select(id),
            )
        })
        .collect();
    rows.push(("Cancel".to_string(), SkillMenuAction::Cancel));
    rows
}

/// Resolves one entry id to its display label for the offer's kind, falling back
/// to the raw id when the catalog, inventory slot, or name is unavailable.
fn entry_label(
    kind: SkillMenuKind,
    id: u32,
    inventory: &Inventory,
    item_db: Option<&ItemDb>,
    skills: Option<&SkillCatalog>,
) -> String {
    match kind {
        SkillMenuKind::Skills => skills
            .and_then(|catalog| catalog.get(id))
            .map(|meta| meta.display_name.clone())
            .unwrap_or_else(|| format!("Skill #{id}")),
        SkillMenuKind::Items => item_name(id, true, item_db),
        SkillMenuKind::InventorySlots => inventory
            .get(id as u16)
            .map(|item| {
                let name = item_name(item.item_id, item.identified, item_db);
                if item.refine > 0 {
                    format!("+{} {}", item.refine, name)
                } else {
                    name
                }
            })
            .unwrap_or_else(|| format!("Slot #{id}")),
    }
}

fn item_name(item_id: u32, identified: bool, item_db: Option<&ItemDb>) -> String {
    item_db
        .and_then(|db| db.name(item_id, identified))
        .map(str::to_string)
        .unwrap_or_else(|| format!("Item #{item_id}"))
}

/// The catalyst row: the pickable chips plus a `Clear` button once anything is
/// picked. `Clear` survives a full selection, which is the only way back from
/// three picks (the chips are gone by then).
fn catalyst_buttons(
    menu: &ActiveSkillMenu,
    inventory: &Inventory,
    item_db: Option<&ItemDb>,
) -> Vec<(String, SkillMenuAction)> {
    let mut buttons = catalyst_chips(menu, inventory, item_db);
    if !menu.catalysts.is_empty() {
        buttons.push(("Clear".to_string(), SkillMenuAction::ClearCatalysts));
    }
    buttons
}

/// The catalyst chips to offer: only for weapon forging, only ids the player
/// actually holds, and only while there is room left in the selection. Each
/// chip's label carries how many of that catalyst remain unspent.
fn catalyst_chips(
    menu: &ActiveSkillMenu,
    inventory: &Inventory,
    item_db: Option<&ItemDb>,
) -> Vec<(String, SkillMenuAction)> {
    if !FORGE_SKILL_IDS.contains(&menu.src_skill_id) || menu.catalysts.len() >= MAX_CATALYSTS {
        return Vec::new();
    }
    CATALYST_IDS
        .iter()
        .filter_map(|&id| {
            let spare = held_amount(inventory, id).saturating_sub(picked_amount(menu, id));
            (spare > 0).then(|| {
                (
                    format!("{} ({spare})", item_name(id, true, item_db)),
                    SkillMenuAction::AddCatalyst(id),
                )
            })
        })
        .collect()
}

/// The catalyst line under the title, or `None` when the menu has no catalyst
/// row at all (every non-forge menu).
fn catalyst_summary(catalysts: &[u32], item_db: Option<&ItemDb>) -> Option<String> {
    (!catalysts.is_empty()).then(|| {
        let names: Vec<_> = catalysts
            .iter()
            .map(|&id| item_name(id, true, item_db))
            .collect();
        format!("Catalysts: {}", names.join(", "))
    })
}

fn held_amount(inventory: &Inventory, item_id: u32) -> u16 {
    inventory
        .iter()
        .filter(|item| item.item_id == item_id)
        .map(|item| item.amount)
        .sum()
}

fn picked_amount(menu: &ActiveSkillMenu, item_id: u32) -> u16 {
    menu.catalysts.iter().filter(|&&id| id == item_id).count() as u16
}

/// Every row's `Activate` handler: selecting or cancelling answers the server and
/// closes the menu; the catalyst rows only mutate the pending selection, which
/// rebuilds the window.
fn on_menu_row(
    activate: On<Activate>,
    actions: Query<&SkillMenuAction>,
    mut menu: ResMut<ActiveSkillMenu>,
    mut commands: Commands,
    mut answers: MessageWriter<AnswerSkillMenu>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    if let Some(answer) = apply_row(*action, &mut menu) {
        answers.write(answer);
        commands.remove_resource::<ActiveSkillMenu>();
    }
}

/// Applies one row click to the pending menu. Returns the answer to send when
/// the click ends the menu (`Select`/`Cancel`), or `None` when it only edited the
/// catalyst selection. Extra picks past [`MAX_CATALYSTS`] are ignored rather than
/// rotating the selection — the chips are hidden at that point, so this only
/// guards against a stale click.
fn apply_row(action: SkillMenuAction, menu: &mut ActiveSkillMenu) -> Option<AnswerSkillMenu> {
    match action {
        SkillMenuAction::Select(id) => Some(AnswerSkillMenu {
            src_skill_id: menu.src_skill_id,
            selected_id: id,
            extra_ids: menu.catalysts.clone(),
            cancel: false,
        }),
        SkillMenuAction::Cancel => Some(cancel_answer(menu)),
        SkillMenuAction::AddCatalyst(id) => {
            if menu.catalysts.len() < MAX_CATALYSTS {
                menu.catalysts.push(id);
            }
            None
        }
        SkillMenuAction::ClearCatalysts => {
            menu.catalysts.clear();
            None
        }
    }
}

/// ESC answers with the reply's `cancel` flag and closes, exactly like the
/// `Cancel` row. `escape_menu` skips its own toggle while this resource exists,
/// so one press never both cancels the menu and opens the escape menu.
fn cancel_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    menu: Res<ActiveSkillMenu>,
    mut commands: Commands,
    mut answers: MessageWriter<AnswerSkillMenu>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    answers.write(cancel_answer(&menu));
    commands.remove_resource::<ActiveSkillMenu>();
}

/// Backing out is its own flag rather than a reserved `selected_id`: inventory-slot
/// menus offer slot `0`, so any id sentinel would swallow a real selection.
fn cancel_answer(menu: &ActiveSkillMenu) -> AnswerSkillMenu {
    AnswerSkillMenu {
        src_skill_id: menu.src_skill_id,
        selected_id: 0,
        extra_ids: Vec::new(),
        cancel: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::domain::inventory::Item;
    use lifthrasir_data::{ItemData, ItemInfo, SkillData, SkillMeta};

    fn item_db() -> ItemDb {
        let mut data = ItemData::default();
        for (id, name) in [
            (1201u32, "Knife"),
            (1000, "Star Crumb"),
            (994, "Flame Heart"),
            (995, "Mystic Frozen"),
            (7620, "Enriched Elunium"),
        ] {
            data.items.insert(
                id,
                ItemInfo {
                    identified_name: name.to_string(),
                    unidentified_name: "Unknown Item".to_string(),
                    ..Default::default()
                },
            );
        }
        ItemDb::from_item_data(data)
    }

    fn skill_catalog() -> SkillCatalog {
        let mut data = SkillData::default();
        data.skills.insert(
            98,
            SkillMeta {
                name: "BS_DAGGER".to_string(),
                display_name: "Dagger Forging".to_string(),
                description: vec![],
                max_level: 3,
                sp_cost: vec![],
                attack_range: vec![],
            },
        );
        SkillCatalog::from_skill_data(data)
    }

    fn inventory_with(items: Vec<Item>) -> Inventory {
        let mut inventory = Inventory::default();
        for item in items {
            inventory.upsert(item);
        }
        inventory
    }

    fn menu(kind: SkillMenuKind, entry_ids: Vec<u32>, catalysts: Vec<u32>) -> ActiveSkillMenu {
        ActiveSkillMenu {
            src_skill_id: 98,
            kind,
            entry_ids,
            catalysts,
        }
    }

    #[test]
    fn item_entries_use_the_item_name() {
        let rows = entry_rows(
            &menu(SkillMenuKind::Items, vec![1201], vec![]),
            &Inventory::default(),
            Some(&item_db()),
            None,
        );

        assert_eq!(rows[0].0, "Knife");
        assert_eq!(rows[0].1, SkillMenuAction::Select(1201));
    }

    #[test]
    fn unknown_item_entries_fall_back_to_the_id() {
        let rows = entry_rows(
            &menu(SkillMenuKind::Items, vec![4242], vec![]),
            &Inventory::default(),
            Some(&item_db()),
            None,
        );

        assert_eq!(rows[0].0, "Item #4242");
    }

    #[test]
    fn skill_entries_use_the_display_name() {
        let rows = entry_rows(
            &menu(SkillMenuKind::Skills, vec![98], vec![]),
            &Inventory::default(),
            None,
            Some(&skill_catalog()),
        );

        assert_eq!(rows[0].0, "Dagger Forging");
    }

    #[test]
    fn inventory_slot_entries_resolve_through_the_slot() {
        let inventory = inventory_with(vec![Item {
            index: 5,
            item_id: 1201,
            amount: 1,
            refine: 7,
            identified: true,
            ..Default::default()
        }]);

        let rows = entry_rows(
            &menu(SkillMenuKind::InventorySlots, vec![5], vec![]),
            &inventory,
            Some(&item_db()),
            None,
        );

        assert_eq!(rows[0].0, "+7 Knife");
        assert_eq!(rows[0].1, SkillMenuAction::Select(5));
    }

    #[test]
    fn inventory_slot_entries_use_the_unidentified_name() {
        let inventory = inventory_with(vec![Item {
            index: 5,
            item_id: 1201,
            amount: 1,
            identified: false,
            ..Default::default()
        }]);

        let rows = entry_rows(
            &menu(SkillMenuKind::InventorySlots, vec![5], vec![]),
            &inventory,
            Some(&item_db()),
            None,
        );

        assert_eq!(rows[0].0, "Unknown Item");
    }

    #[test]
    fn missing_inventory_slot_falls_back_to_the_slot_id() {
        let rows = entry_rows(
            &menu(SkillMenuKind::InventorySlots, vec![9], vec![]),
            &Inventory::default(),
            Some(&item_db()),
            None,
        );

        assert_eq!(rows[0].0, "Slot #9");
    }

    #[test]
    fn every_offer_ends_with_cancel() {
        let rows = entry_rows(
            &menu(SkillMenuKind::Items, vec![], vec![]),
            &Inventory::default(),
            Some(&item_db()),
            None,
        );

        assert_eq!(rows, vec![("Cancel".to_string(), SkillMenuAction::Cancel)]);
    }

    #[test]
    fn catalyst_chips_offer_only_held_catalysts() {
        let inventory = inventory_with(vec![
            Item {
                index: 1,
                item_id: 1000,
                amount: 2,
                identified: true,
                ..Default::default()
            },
            Item {
                index: 2,
                item_id: 7620,
                amount: 5,
                identified: true,
                ..Default::default()
            },
        ]);

        let chips = catalyst_chips(
            &menu(SkillMenuKind::Items, vec![1201], vec![]),
            &inventory,
            Some(&item_db()),
        );

        assert_eq!(
            chips,
            vec![(
                "Star Crumb (2)".to_string(),
                SkillMenuAction::AddCatalyst(1000)
            )]
        );
    }

    #[test]
    fn catalyst_chips_discount_already_picked_ones() {
        let inventory = inventory_with(vec![Item {
            index: 1,
            item_id: 1000,
            amount: 2,
            identified: true,
            ..Default::default()
        }]);

        let chips = catalyst_chips(
            &menu(SkillMenuKind::Items, vec![1201], vec![1000]),
            &inventory,
            Some(&item_db()),
        );

        assert_eq!(chips[0].0, "Star Crumb (1)");

        let spent = catalyst_chips(
            &menu(SkillMenuKind::Items, vec![1201], vec![1000, 1000]),
            &inventory,
            Some(&item_db()),
        );

        assert!(spent.is_empty());
    }

    #[test]
    fn catalyst_chips_stop_at_three_picks() {
        let inventory = inventory_with(vec![Item {
            index: 1,
            item_id: 1000,
            amount: 9,
            identified: true,
            ..Default::default()
        }]);

        let chips = catalyst_chips(
            &menu(SkillMenuKind::Items, vec![1201], vec![1000, 1000, 1000]),
            &inventory,
            Some(&item_db()),
        );

        assert!(chips.is_empty());
    }

    #[test]
    fn non_forge_skills_offer_no_catalysts() {
        let inventory = inventory_with(vec![Item {
            index: 1,
            item_id: 1000,
            amount: 9,
            identified: true,
            ..Default::default()
        }]);
        let mut pharmacy = menu(SkillMenuKind::Items, vec![501], vec![]);
        pharmacy.src_skill_id = 228;

        assert!(catalyst_chips(&pharmacy, &inventory, Some(&item_db())).is_empty());
    }

    #[test]
    fn catalyst_summary_lists_picks_in_order() {
        assert_eq!(
            catalyst_summary(&[1000, 994], Some(&item_db())),
            Some("Catalysts: Star Crumb, Flame Heart".to_string())
        );
        assert_eq!(catalyst_summary(&[], Some(&item_db())), None);
    }

    #[test]
    fn menu_title_falls_back_without_a_catalog() {
        assert_eq!(menu_title(98, Some(&skill_catalog())), "Dagger Forging");
        assert_eq!(menu_title(98, None), "Select");
        assert_eq!(menu_title(4242, Some(&skill_catalog())), "Select");
    }

    #[test]
    fn cancel_sets_the_cancel_flag_without_catalysts() {
        assert_eq!(
            cancel_answer(&menu(SkillMenuKind::Items, vec![1201], vec![1000])),
            AnswerSkillMenu {
                src_skill_id: 98,
                selected_id: 0,
                extra_ids: vec![],
                cancel: true,
            }
        );
    }

    #[test]
    fn clear_button_appears_once_a_catalyst_is_picked() {
        let inventory = inventory_with(vec![Item {
            index: 1,
            item_id: 1000,
            amount: 9,
            identified: true,
            ..Default::default()
        }]);

        let none_picked = catalyst_buttons(
            &menu(SkillMenuKind::Items, vec![1201], vec![]),
            &inventory,
            Some(&item_db()),
        );
        assert!(
            !none_picked
                .iter()
                .any(|(_, action)| *action == SkillMenuAction::ClearCatalysts)
        );

        // Still reachable at a full selection, where the chips are gone.
        let full = catalyst_buttons(
            &menu(SkillMenuKind::Items, vec![1201], vec![1000, 1000, 1000]),
            &inventory,
            Some(&item_db()),
        );
        assert_eq!(
            full,
            vec![("Clear".to_string(), SkillMenuAction::ClearCatalysts)]
        );
    }

    #[test]
    fn selecting_answers_with_the_picked_catalysts() {
        let mut pending = menu(SkillMenuKind::Items, vec![1201], vec![1000, 994]);

        assert_eq!(
            apply_row(SkillMenuAction::Select(1201), &mut pending),
            Some(AnswerSkillMenu {
                src_skill_id: 98,
                selected_id: 1201,
                extra_ids: vec![1000, 994],
                cancel: false,
            })
        );
    }

    // Inventory-slot menus offer container keys, and the first slot is 0. Selecting
    // it has to answer with `cancel: false` or the server reads it as backing out.
    #[test]
    fn selecting_inventory_slot_zero_is_not_a_cancel() {
        let mut pending = menu(SkillMenuKind::InventorySlots, vec![0], vec![]);

        assert_eq!(
            apply_row(SkillMenuAction::Select(0), &mut pending),
            Some(AnswerSkillMenu {
                src_skill_id: 98,
                selected_id: 0,
                extra_ids: vec![],
                cancel: false,
            })
        );
    }

    #[test]
    fn catalyst_rows_edit_the_selection_without_answering() {
        let mut pending = menu(SkillMenuKind::Items, vec![1201], vec![]);

        assert_eq!(
            apply_row(SkillMenuAction::AddCatalyst(1000), &mut pending),
            None
        );
        assert_eq!(pending.catalysts, vec![1000]);

        assert_eq!(
            apply_row(SkillMenuAction::ClearCatalysts, &mut pending),
            None
        );
        assert!(pending.catalysts.is_empty());
    }

    #[test]
    fn a_fourth_catalyst_pick_is_ignored() {
        let mut pending = menu(SkillMenuKind::Items, vec![1201], vec![1000, 1000, 1000]);

        apply_row(SkillMenuAction::AddCatalyst(994), &mut pending);

        assert_eq!(pending.catalysts, vec![1000, 1000, 1000]);
    }

    #[test]
    fn an_offer_opens_a_menu_with_no_catalysts_picked() {
        let mut app = App::new();
        app.add_message::<SkillMenuOffered>()
            .add_systems(Update, open_menu);

        app.world_mut()
            .resource_mut::<Messages<SkillMenuOffered>>()
            .write(SkillMenuOffered {
                src_skill_id: 98,
                kind: SkillMenuKind::Items,
                entry_ids: vec![1201, 1202],
            });
        app.update();

        assert_eq!(
            app.world().get_resource::<ActiveSkillMenu>(),
            Some(&menu(SkillMenuKind::Items, vec![1201, 1202], vec![]))
        );
    }

    #[test]
    fn a_newer_offer_replaces_the_pending_one() {
        let mut app = App::new();
        app.add_message::<SkillMenuOffered>()
            .add_systems(Update, open_menu)
            .insert_resource(menu(SkillMenuKind::Items, vec![1201], vec![1000]));

        app.world_mut()
            .resource_mut::<Messages<SkillMenuOffered>>()
            .write(SkillMenuOffered {
                src_skill_id: 40,
                kind: SkillMenuKind::InventorySlots,
                entry_ids: vec![3],
            });
        app.update();

        let active = app.world().resource::<ActiveSkillMenu>();
        assert_eq!(active.src_skill_id, 40);
        assert_eq!(active.kind, SkillMenuKind::InventorySlots);
        assert_eq!(active.entry_ids, vec![3]);
        assert!(active.catalysts.is_empty());
    }
}
