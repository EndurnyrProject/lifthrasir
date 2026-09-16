//! Info modal: a message-driven right-click inspect popup for items and skills
//! (BSN + Feathers), ported from the Endurnir `info-modals.css` mockups.
//!
//! Any surface (bag, equipment, skills, guild, storage, shop, cart) summons it by
//! building an [`InfoContent`] from the data it already has and writing
//! [`ShowInfoModal`]; `show_info_modal` despawns any modal already open and spawns a
//! fresh one — rebuild-on-show, which is also how requirement-chip navigation works
//! in the skill scene. The modal never resolves ids against domain resources itself,
//! so adding a surface touches only that surface. Unlike
//! [`system_dialog`](super::system_dialog), the backdrop itself closes the modal on
//! click, in addition to the close button and Escape.
//!
//! This module only owns the shell and lifecycle; [`shell`] holds the shared chrome
//! scenes, [`view`] the builders that turn domain payloads into view structs, and
//! item/skill content lives in `item_scene`/`skill_scene`.

use bevy::prelude::*;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};

use game_engine::domain::inventory::{Item, ItemCategory};

use crate::theme::feathers_theme::install_norse_theme;
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

/// What the modal shows, built by the summoning surface.
#[derive(Debug, Clone, PartialEq)]
pub enum InfoContent {
    /// `raise` is the tree skill id the footer's Raise button stages; `None`
    /// renders info only (guild skills are raised from the guild window).
    Skill {
        view: view::SkillInfoView,
        raise: Option<u32>,
    },
    /// `action` is the footer's primary button; `None` when the summoning context
    /// has no valid one (storage, cart, shop).
    Item {
        view: view::ItemInfoView,
        action: Option<ItemAction>,
    },
}

/// The item footer's primary action on the inventory slot at `index`. The click
/// revalidates the slot still holds the view's `item_id` before acting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemAction {
    pub kind: ItemActionKind,
    pub index: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemActionKind {
    Use,
    Equip,
    Unequip,
}

impl ItemAction {
    /// The primary action for an item sitting in the bag: Use or Equip by
    /// category, none for Etc.
    pub fn for_bag_item(item: &Item) -> Option<Self> {
        let kind = match item.category() {
            ItemCategory::Use => ItemActionKind::Use,
            ItemCategory::Equip => ItemActionKind::Equip,
            ItemCategory::Etc => return None,
        };
        Some(Self {
            kind,
            index: item.index,
        })
    }

    pub fn unequip(index: u16) -> Self {
        Self {
            kind: ItemActionKind::Unequip,
            index,
        }
    }
}

/// Opens the info modal with `content`, replacing any modal already open.
#[derive(Message, Debug, Clone)]
pub struct ShowInfoModal {
    pub content: InfoContent,
}

/// The modal root. A fresh one is spawned on every show, so at most one exists.
#[derive(Component, Default, Clone)]
pub struct InfoModalRoot;

/// Spawns the modal for the latest request, despawning any modal already open.
/// Last message wins when several are written in one frame (e.g. a requirement-chip
/// click that rebuilds the modal for a different skill).
pub(crate) fn show_info_modal(
    mut requests: MessageReader<ShowInfoModal>,
    existing: Query<Entity, With<InfoModalRoot>>,
    mut commands: Commands,
) {
    let Some(request) = requests.read().last() else {
        return;
    };
    despawn_existing(&existing, &mut commands);
    match request.content.clone() {
        InfoContent::Skill { view, raise } => {
            commands.spawn_scene(info_modal(view.edge, skill_scene::scene(view, raise)));
        }
        InfoContent::Item { view, action } => {
            commands.spawn_scene(info_modal(view.edge, item_scene::scene(view, action)));
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

    fn skill(name: &str) -> ShowInfoModal {
        ShowInfoModal {
            content: InfoContent::Skill {
                view: view::SkillInfoView {
                    icon_path: None,
                    edge: shell::EdgeGrade::Fine,
                    name: name.to_string(),
                    kind: "Active".to_string(),
                    level_line: "1/5".to_string(),
                    description: vec![],
                    sp_cost: None,
                    range: None,
                    requires: vec![],
                    unlocks: vec![],
                    can_raise: false,
                    points_left: 0,
                },
                raise: Some(1),
            },
        }
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.init_resource::<ButtonInput<KeyCode>>();
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
        app.world_mut().write_message(skill("Bash"));
        app.update();

        assert_eq!(roots(&mut app).len(), 1);
    }

    #[test]
    fn showing_again_replaces_the_root() {
        let mut app = test_app();
        app.world_mut().write_message(skill("Bash"));
        app.update();
        let first = roots(&mut app)[0];

        app.world_mut().write_message(skill("Provoke"));
        app.update();

        let after = roots(&mut app);
        assert_eq!(after.len(), 1);
        assert_ne!(after[0], first);
    }

    #[test]
    fn last_request_in_a_frame_wins() {
        let mut app = test_app();
        app.world_mut().write_message(skill("Bash"));
        app.world_mut().write_message(skill("Provoke"));
        app.update();

        assert_eq!(roots(&mut app).len(), 1);
        let texts: Vec<String> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        assert!(texts.contains(&"Provoke".to_string()), "{texts:?}");
        assert!(!texts.contains(&"Bash".to_string()), "{texts:?}");
    }

    #[test]
    fn bag_item_action_follows_category() {
        let usable = Item {
            index: 3,
            item_type: 0,
            ..Default::default()
        };
        let equip = Item {
            index: 4,
            item_type: 5,
            ..Default::default()
        };
        let etc = Item {
            index: 5,
            item_type: 3,
            ..Default::default()
        };
        assert_eq!(
            ItemAction::for_bag_item(&usable),
            Some(ItemAction {
                kind: ItemActionKind::Use,
                index: 3
            })
        );
        assert_eq!(
            ItemAction::for_bag_item(&equip),
            Some(ItemAction {
                kind: ItemActionKind::Equip,
                index: 4
            })
        );
        assert_eq!(ItemAction::for_bag_item(&etc), None);
    }

    #[test]
    fn backdrop_is_pickable_so_clicks_do_not_leak_to_the_world() {
        let mut app = test_app();
        app.world_mut().write_message(skill("Bash"));
        app.update();

        let root = roots(&mut app)[0];
        assert!(app.world().get::<Pickable>(root).is_some());
    }

    #[test]
    fn escape_despawns_the_open_modal() {
        let mut app = test_app();
        app.world_mut().write_message(skill("Bash"));
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
