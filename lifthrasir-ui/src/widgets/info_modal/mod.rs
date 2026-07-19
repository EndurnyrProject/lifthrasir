//! Info modal: a message-driven right-click inspect popup for items and skills
//! (BSN + Feathers), ported from the Endurnir `info-modals.css` mockups.
//!
//! Any surface (bag, equipment, skills, storage, shop, cart) summons it by writing
//! [`ShowInfoModal`]; `show_info_modal` despawns any modal already open and spawns a
//! fresh one — rebuild-on-show, which is also how requirement-chip navigation works
//! in the skill scene (a later task). Unlike [`system_dialog`](super::system_dialog),
//! the backdrop itself closes the modal on click, in addition to the close button
//! and Escape.
//!
//! This module only owns the shell and lifecycle; [`shell`] holds the shared chrome
//! scenes and item/skill content is added by later scenes dispatched from
//! `show_info_modal`.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy_feathers::theme::ThemeTextColor;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};

use crate::theme;
use crate::theme::feathers_theme::{install_norse_theme, TOKEN_TEXT_DIM};
use crate::widgets::storage_window::StorageSelection;
use crate::widgets::system_dialog;

pub mod shell;

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
        app.add_systems(Update, (show_info_modal, close_on_escape));
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
fn show_info_modal(
    mut requests: MessageReader<ShowInfoModal>,
    existing: Query<Entity, With<InfoModalRoot>>,
    mut commands: Commands,
) {
    let Some(request) = requests.read().last() else {
        return;
    };
    for root in &existing {
        commands.entity(root).despawn();
    }
    commands.spawn_scene(info_modal(request.target));
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
fn info_modal(target: InfoTarget) -> impl Scene {
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
        Children [ shell::card(shell::EdgeGrade::default(), placeholder_body(target)) ]
    }
}

/// Task-1 placeholder body; item/skill scenes replace this once their view models
/// and content scenes exist (later tasks).
fn placeholder_body(target: InfoTarget) -> impl Scene {
    let text = match target {
        InfoTarget::Skill(id) => format!("Skill #{id}"),
        InfoTarget::Item(item_ref) => format!("Item {item_ref:?}"),
    };
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            padding: {UiRect::axes(px(20), px(20))},
        }
        Children [
            (
                Text(text)
                TextFont {
                    font: FontSourceTemplate::Handle(theme::FONT_BODY),
                    font_size: {FontSize::Px(13.0)},
                }
                ThemeTextColor({TOKEN_TEXT_DIM})
                Pickable { should_block_lower: false, is_hoverable: false }
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;

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
            target: InfoTarget::Item(ItemRef::Inventory(3)),
        });
        app.update();

        let after = roots(&mut app);
        assert_eq!(after.len(), 1);
        assert_ne!(after[0], first);
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
