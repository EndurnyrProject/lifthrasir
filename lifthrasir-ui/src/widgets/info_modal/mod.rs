//! Info modal: a message-driven right-click inspect popup for items and skills
//! (BSN + Feathers), ported from the Endurnir `info-modals.css` mockups.
//!
//! Any surface (bag, equipment, skills, storage, shop, cart) summons it by writing
//! [`ShowInfoModal`]; `show_info_modal` despawns any modal already open and spawns a
//! fresh one — rebuild-on-show, which is also how requirement-chip navigation works
//! in the skill scene. Unlike [`system_dialog`](super::system_dialog), the backdrop
//! itself closes the modal on click, in addition to the close button and Escape.
//!
//! This module only owns the shell and lifecycle; [`shell`] holds the shared chrome
//! scenes and item/skill content lives in `item_scene`/`skill_scene`, dispatched
//! from `show_info_modal`.

use bevy::prelude::*;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};

use game_engine::domain::cart::Cart;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::inventory::Inventory;
use game_engine::domain::skill::SkillTreeState;
use game_engine::domain::storage::Storage;
use game_engine::infrastructure::item::ItemDb;
use game_engine::infrastructure::skill::SkillCatalog;

use crate::theme::feathers_theme::install_norse_theme;
use crate::widgets::character_window::SkillPanelStaging;
use crate::widgets::shop_window::ShopSession;
use crate::widgets::storage_window::StorageSelection;
use crate::widgets::system_dialog;

mod item_scene;
pub mod shell;
mod skill_scene;
pub mod view;

/// Sits one tier below the system dialog, so a confirm/disconnect dialog always
/// stacks above and stays clickable.
pub const INFO_MODAL_Z: i32 = system_dialog::DIALOG_Z - 1;

pub struct InfoModalPlugin;

impl Plugin for InfoModalPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.add_message::<ShowInfoModal>();
        app.add_systems(
            Update,
            (
                show_info_modal,
                close_on_escape,
                item_scene::apply_footer_disabled.after(show_info_modal),
                skill_scene::apply_raise_disabled.after(show_info_modal),
            ),
        );
    }
}

/// Right-click inspect target: an item ref (per surface) or a skill id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoTarget {
    Skill(u32),
    Item(ItemRef),
}

/// Where the inspected item lives, so a later task's footer can act on (and
/// revalidate) it at the stored index/selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemRef {
    Inventory(u16),
    Equipped(u16),
    Storage(StorageSelection),
    Cart(u16),
    ShopBuy(u32),
}

/// Opens the info modal for `target`, replacing any modal already open.
#[derive(Message, Debug, Clone)]
pub struct ShowInfoModal {
    pub target: InfoTarget,
}

/// The modal root. A fresh one is spawned on every show, so at most one exists.
#[derive(Component, Default, Clone)]
pub struct InfoModalRoot;

/// Spawns the modal for the latest request, despawning any modal already open.
/// Last message wins when several are written in one frame (e.g. a requirement-chip
/// click that rebuilds the modal for a different skill).
///
/// Built before despawning the existing root, so a request that can't be honored
/// (registry not loaded, or the item ref no longer resolves) leaves an already-open
/// modal untouched instead of replacing it with nothing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn show_info_modal(
    mut requests: MessageReader<ShowInfoModal>,
    existing: Query<Entity, With<InfoModalRoot>>,
    item_db: Option<Res<ItemDb>>,
    inventory: Res<Inventory>,
    storage: Res<Storage>,
    cart: Res<Cart>,
    shop: Option<Res<ShopSession>>,
    skill_catalog: Option<Res<SkillCatalog>>,
    skill_tree: Res<SkillTreeState>,
    skill_staging: Res<SkillPanelStaging>,
    local_player: Query<&CharacterStatus, With<LocalPlayer>>,
    mut commands: Commands,
) {
    let Some(request) = requests.read().last() else {
        return;
    };
    match request.target {
        InfoTarget::Skill(id) => {
            let Some(catalog) = skill_catalog.as_deref() else {
                warn!("info modal: SkillCatalog not loaded yet, ignoring show request");
                return;
            };
            let status = local_player.single().ok();
            let Some(view) =
                view::build_skill_view(id, Some(catalog), &skill_tree, &skill_staging, status)
            else {
                warn!("info modal: skill #{id} not in the tree, ignoring show request");
                return;
            };
            despawn_existing(&existing, &mut commands);
            commands.spawn_scene(info_modal(view.edge, skill_scene::scene(view, id)));
        }
        InfoTarget::Item(item_ref) => {
            let Some(item_db) = item_db.as_deref() else {
                warn!("info modal: ItemDb not loaded yet, ignoring show request");
                return;
            };
            let Some(view) = view::build_item_view(
                item_ref,
                item_db,
                &inventory,
                &storage,
                &cart,
                shop.as_deref(),
            ) else {
                warn!(
                    "info modal: {item_ref:?} no longer resolves to an item, ignoring show request"
                );
                return;
            };
            let category = match item_ref {
                ItemRef::Inventory(index) | ItemRef::Equipped(index) => {
                    inventory.get(index).map(|item| item.category())
                }
                ItemRef::Storage(_) | ItemRef::Cart(_) | ItemRef::ShopBuy(_) => None,
            };
            despawn_existing(&existing, &mut commands);
            commands.spawn_scene(info_modal(
                view.edge,
                item_scene::scene(view, item_ref, category),
            ));
        }
    }
}

fn despawn_existing(existing: &Query<Entity, With<InfoModalRoot>>, commands: &mut Commands) {
    for root in existing {
        commands.entity(root).despawn();
    }
}

/// Escape closes the modal, gated on a root existing so it never swallows the key
/// otherwise. Nothing else consumes Escape while the modal is open today; if a
/// future consumer appears, order it against this system.
fn close_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    root: Query<Entity, With<InfoModalRoot>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    let Ok(root) = root.single() else {
        return;
    };
    commands.entity(root).despawn();
}

/// Backdrop click closes the modal — deliberately unlike `system_dialog`, whose
/// backdrop is not clickable. The card scene stops click propagation, so this only
/// fires for clicks outside it.
fn close_on_backdrop_click(
    _: On<Pointer<Click>>,
    root: Query<Entity, With<InfoModalRoot>>,
    mut commands: Commands,
) {
    if let Ok(root) = root.single() {
        commands.entity(root).despawn();
    }
}

/// The whole modal as one scene: a dimmed, click-eating backdrop centering the card.
fn info_modal(edge: shell::EdgeGrade, body: impl Scene) -> impl Scene {
    bsn! {
        InfoModalRoot
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
        }
        BackgroundColor({Color::srgba(0.016, 0.031, 0.027, 0.5)})
        GlobalZIndex({INFO_MODAL_Z})
        Pickable
        on(close_on_backdrop_click)
        Children [ shell::card(edge, body) ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;

    fn skill_tree() -> SkillTreeState {
        use game_engine::domain::skill::SkillNode;
        let mut skills = std::collections::HashMap::new();
        for id in [1, 2] {
            skills.insert(
                id,
                SkillNode {
                    level: 0,
                    max_level: 5,
                    upgradable: true,
                    requires: vec![],
                    req_base_level: 0,
                    req_job_level: 0,
                    sp: 1,
                    range: 1,
                    inf_type: 0,
                    job_id: 1,
                    splash_radius: 0,
                },
            );
        }
        SkillTreeState { skills }
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<Inventory>();
        app.init_resource::<Storage>();
        app.init_resource::<Cart>();
        app.insert_resource(skill_tree());
        app.insert_resource(SkillCatalog::from_skill_data(
            lifthrasir_data::SkillData::default(),
        ));
        app.init_resource::<SkillPanelStaging>();
        app.add_message::<ShowInfoModal>();
        app.add_systems(Update, (show_info_modal, close_on_escape));
        app
    }

    fn roots(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<InfoModalRoot>>()
            .iter(app.world())
            .collect()
    }

    #[test]
    fn showing_spawns_exactly_one_root() {
        let mut app = test_app();
        app.world_mut().write_message(ShowInfoModal {
            target: InfoTarget::Skill(1),
        });
        app.update();

        assert_eq!(roots(&mut app).len(), 1);
    }

    #[test]
    fn showing_again_replaces_the_root() {
        let mut app = test_app();
        app.world_mut().write_message(ShowInfoModal {
            target: InfoTarget::Skill(1),
        });
        app.update();
        let first = roots(&mut app)[0];

        app.world_mut().write_message(ShowInfoModal {
            target: InfoTarget::Skill(2),
        });
        app.update();

        let after = roots(&mut app);
        assert_eq!(after.len(), 1);
        assert_ne!(after[0], first);
    }

    #[test]
    fn skill_target_with_no_catalog_ignores_the_request() {
        let mut app = test_app();
        app.world_mut().remove_resource::<SkillCatalog>();
        app.world_mut().write_message(ShowInfoModal {
            target: InfoTarget::Skill(1),
        });
        app.update();

        assert!(roots(&mut app).is_empty());
    }

    #[test]
    fn skill_target_not_in_the_tree_ignores_the_request() {
        let mut app = test_app();
        app.world_mut().write_message(ShowInfoModal {
            target: InfoTarget::Skill(9999),
        });
        app.update();

        assert!(roots(&mut app).is_empty());
    }

    #[test]
    fn item_target_with_no_item_db_ignores_the_request() {
        let mut app = test_app();
        app.world_mut().write_message(ShowInfoModal {
            target: InfoTarget::Item(ItemRef::Inventory(3)),
        });
        app.update();

        assert!(roots(&mut app).is_empty());
    }

    #[test]
    fn item_target_with_an_empty_slot_ignores_the_request() {
        let mut app = test_app();
        app.insert_resource(ItemDb::default());
        app.world_mut().write_message(ShowInfoModal {
            target: InfoTarget::Item(ItemRef::Inventory(3)),
        });
        app.update();

        assert!(roots(&mut app).is_empty());
    }

    #[test]
    fn item_target_resolves_and_spawns_a_root() {
        let mut app = test_app();
        app.insert_resource(ItemDb::default());
        app.world_mut()
            .resource_mut::<Inventory>()
            .upsert(game_engine::domain::inventory::Item {
                index: 3,
                item_id: 501,
                identified: true,
                ..Default::default()
            });
        app.world_mut().write_message(ShowInfoModal {
            target: InfoTarget::Item(ItemRef::Inventory(3)),
        });
        app.update();

        assert_eq!(roots(&mut app).len(), 1);
    }

    #[test]
    fn backdrop_is_pickable_so_clicks_do_not_leak_to_the_world() {
        let mut app = test_app();
        app.world_mut().write_message(ShowInfoModal {
            target: InfoTarget::Skill(1),
        });
        app.update();

        let root = roots(&mut app)[0];
        assert!(app.world().get::<Pickable>(root).is_some());
    }

    #[test]
    fn escape_despawns_the_open_modal() {
        let mut app = test_app();
        app.world_mut().write_message(ShowInfoModal {
            target: InfoTarget::Skill(1),
        });
        app.update();
        assert_eq!(roots(&mut app).len(), 1);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        assert!(roots(&mut app).is_empty());
    }

    #[test]
    fn escape_without_an_open_modal_is_a_no_op() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        assert!(roots(&mut app).is_empty());
    }

    fn click_event(target: Entity, window: Entity) -> Pointer<Click> {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::backend::HitData;
        use bevy::picking::pointer::{Location, PointerId};
        use bevy::window::WindowRef;
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary.normalize(Some(window)).unwrap(),
                ),
                position: Vec2::ZERO,
            },
            Click {
                button: PointerButton::Primary,
                hit: HitData::new(target, 0.0, None, None),
                duration: std::time::Duration::ZERO,
                count: 1,
            },
            target,
        )
    }

    #[test]
    fn backdrop_click_despawns_the_root() {
        let mut app = App::new();
        let root = app
            .world_mut()
            .spawn(InfoModalRoot)
            .observe(close_on_backdrop_click)
            .id();
        let window = app.world_mut().spawn_empty().id();

        app.world_mut().trigger(click_event(root, window));
        app.world_mut().flush();

        assert!(app.world().get_entity(root).is_err());
    }
}
