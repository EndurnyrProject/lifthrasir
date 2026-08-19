//! Right-click context menu for party member rows (Make Leader / Kick). The menu
//! spawns as a top-level overlay without a `ChildOf`, so a party-window body
//! respawn never despawns an open menu.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::{ButtonVariant, FeathersButton};
use bevy_feathers::theme::ThemedText;
use net_contract::commands::{PartyKickRequested, PartyLeaderRequested};

use crate::theme;

const MENU_Z: i32 = i32::MAX - 5;
const MENU_WIDTH: f32 = 168.0;

/// Marks an actionable party-window row and carries the member's char_id.
#[derive(Component, Clone, Default)]
pub struct PartyMemberRow(pub u32);

#[derive(Component, Default, Clone)]
pub struct PartyMemberMenuRoot;

#[derive(Component, Clone, Default)]
pub struct MenuTarget(pub u32);

/// Global (app-wide) observer: a right-click on a row carrying [`PartyMemberRow`]
/// opens the menu for that member. Left-clicks and marker-less rows are ignored.
pub fn open(
    mut click: On<Pointer<Click>>,
    rows: Query<&PartyMemberRow>,
    child_of: Query<&ChildOf>,
    existing: Query<Entity, With<PartyMemberMenuRoot>>,
    mut commands: Commands,
) {
    if click.event.button != PointerButton::Secondary {
        return;
    }
    let Some(target) = resolve_target(click.entity, &rows, &child_of) else {
        return;
    };
    click.propagate(false);
    for menu in &existing {
        commands.entity(menu).despawn();
    }
    commands.spawn_scene(menu(click.pointer_location.position, target));
}

/// Resolve the right-clicked entity to its member char_id: the pickable row entity
/// carries [`PartyMemberRow`] itself, but a pickable child bubbles up one level.
fn resolve_target(
    entity: Entity,
    rows: &Query<&PartyMemberRow>,
    child_of: &Query<&ChildOf>,
) -> Option<u32> {
    if let Ok(row) = rows.get(entity) {
        return Some(row.0);
    }
    let parent = child_of.get(entity).ok()?.parent();
    rows.get(parent).ok().map(|row| row.0)
}

fn menu(cursor: Vec2, target: u32) -> impl Scene {
    bsn! {
        PartyMemberMenuRoot
        MenuTarget({target})
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
        }
        GlobalZIndex({MENU_Z})
        Pickable
        on(dismiss_menu)
        Children [ card(cursor) ]
    }
}

fn card(cursor: Vec2) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: {px(cursor.x)},
            top: {px(cursor.y)},
            width: px(MENU_WIDTH),
            padding: {UiRect::all(px(6))},
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            border: px(1),
            border_radius: BorderRadius::all(px(10)),
        }
        BackgroundColor({theme::GLASS})
        BorderColor::all(theme::STROKE)
        Pickable
        on(|mut click: On<Pointer<Click>>| click.propagate(false))
        Children [
            action_button("Make Leader", on(on_make_leader)),
            action_button("Kick", on(on_kick)),
        ]
    }
}

fn action_button(label: &'static str, observer: impl Scene) -> impl Scene {
    bsn! {
        @FeathersButton {
            @caption: bsn! {
                (
                    Text(label)
                    TextFont {
                        font: FontSourceTemplate::Handle(theme::FONT_BODY),
                        font_size: {FontSize::Px(14.0)},
                    }
                    ThemedText
                )
            },
            @variant: ButtonVariant::Primary,
        }
        Node { height: px(36), border_radius: BorderRadius::all(px(7)) }
        {observer}
    }
}

fn on_make_leader(
    _: On<Activate>,
    menu: Query<(Entity, &MenuTarget), With<PartyMemberMenuRoot>>,
    mut writer: MessageWriter<PartyLeaderRequested>,
    mut commands: Commands,
) {
    let Ok((root, target)) = menu.single() else {
        return;
    };
    writer.write(PartyLeaderRequested {
        target_char_id: target.0,
    });
    commands.entity(root).despawn();
}

fn on_kick(
    _: On<Activate>,
    menu: Query<(Entity, &MenuTarget), With<PartyMemberMenuRoot>>,
    mut writer: MessageWriter<PartyKickRequested>,
    mut commands: Commands,
) {
    let Ok((root, target)) = menu.single() else {
        return;
    };
    writer.write(PartyKickRequested {
        target_char_id: target.0,
    });
    commands.entity(root).despawn();
}

fn dismiss_menu(
    _: On<Pointer<Click>>,
    menu: Query<Entity, With<PartyMemberMenuRoot>>,
    mut commands: Commands,
) {
    if let Ok(root) = menu.single() {
        commands.entity(root).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
    use bevy::picking::backend::HitData;
    use bevy::picking::events::{Click, Pointer};
    use bevy::picking::pointer::{Location, PointerId};
    use bevy::scene::ScenePlugin;
    use std::time::Duration;

    fn pointer_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.add_message::<PartyKickRequested>();
        app.add_message::<PartyLeaderRequested>();
        app.add_observer(open);
        app
    }

    fn pointer_click(target: Entity, button: PointerButton) -> Pointer<Click> {
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Image(ImageRenderTarget {
                    handle: Handle::default(),
                    scale_factor: 1.0,
                }),
                position: Vec2::ZERO,
            },
            Click {
                button,
                hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
                duration: Duration::ZERO,
                count: 1,
            },
            target,
        )
    }

    fn menu_root_count(app: &mut App) -> usize {
        let world = app.world_mut();
        world
            .query_filtered::<(), With<PartyMemberMenuRoot>>()
            .iter(world)
            .count()
    }

    fn menu_target(app: &mut App) -> u32 {
        let world = app.world_mut();
        world.query::<&MenuTarget>().single(world).unwrap().0
    }

    #[test]
    fn right_click_on_an_actionable_row_spawns_exactly_one_menu() {
        let mut app = pointer_app();
        let first = app.world_mut().spawn(PartyMemberRow(77)).id();

        app.world_mut()
            .trigger(pointer_click(first, PointerButton::Secondary));
        app.world_mut().flush();

        assert_eq!(menu_root_count(&mut app), 1);
        assert_eq!(menu_target(&mut app), 77);

        // A second right-click replaces the open menu instead of stacking another.
        let second = app.world_mut().spawn(PartyMemberRow(88)).id();
        app.world_mut()
            .trigger(pointer_click(second, PointerButton::Secondary));
        app.world_mut().flush();

        assert_eq!(menu_root_count(&mut app), 1);
        assert_eq!(menu_target(&mut app), 88);
    }

    #[test]
    fn left_click_on_an_actionable_row_opens_no_menu() {
        let mut app = pointer_app();
        let row = app.world_mut().spawn(PartyMemberRow(77)).id();

        app.world_mut()
            .trigger(pointer_click(row, PointerButton::Primary));
        app.world_mut().flush();

        assert_eq!(menu_root_count(&mut app), 0);
    }

    #[test]
    fn click_resolves_the_row_through_a_pickable_child() {
        let mut app = pointer_app();
        let row = app.world_mut().spawn(PartyMemberRow(9)).id();
        let child = app.world_mut().spawn_empty().insert(ChildOf(row)).id();

        app.world_mut()
            .trigger(pointer_click(child, PointerButton::Secondary));
        app.world_mut().flush();

        assert_eq!(menu_root_count(&mut app), 1);
        assert_eq!(menu_target(&mut app), 9);
    }

    #[test]
    fn right_click_resolves_the_marked_row_even_when_it_has_a_parent() {
        // Real UI shape: the row entity carries the marker and is a child of the
        // body container, so the pick target is the marked row itself, not a child.
        let mut app = pointer_app();
        let body = app.world_mut().spawn_empty().id();
        let row = app
            .world_mut()
            .spawn(PartyMemberRow(42))
            .insert(ChildOf(body))
            .id();

        app.world_mut()
            .trigger(pointer_click(row, PointerButton::Secondary));
        app.world_mut().flush();

        assert_eq!(menu_root_count(&mut app), 1);
        assert_eq!(menu_target(&mut app), 42);
    }

    fn activation_app() -> App {
        let mut app = App::new();
        app.add_message::<PartyKickRequested>();
        app.add_message::<PartyLeaderRequested>();
        app
    }

    #[test]
    fn kick_writes_one_command_for_the_target_and_despawns_the_menu() {
        let mut app = activation_app();
        app.world_mut()
            .spawn((PartyMemberMenuRoot, MenuTarget(1337)));
        let button = app.world_mut().spawn_empty().observe(on_kick).id();

        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        let messages = app.world().resource::<Messages<PartyKickRequested>>();
        let mut cursor = messages.get_cursor();
        let written: Vec<_> = cursor.read(messages).collect();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].target_char_id, 1337);
        assert_eq!(menu_root_count(&mut app), 0);
    }

    #[test]
    fn make_leader_writes_one_command_for_the_target_and_despawns_the_menu() {
        let mut app = activation_app();
        app.world_mut()
            .spawn((PartyMemberMenuRoot, MenuTarget(1337)));
        let button = app.world_mut().spawn_empty().observe(on_make_leader).id();

        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        let messages = app.world().resource::<Messages<PartyLeaderRequested>>();
        let mut cursor = messages.get_cursor();
        let written: Vec<_> = cursor.read(messages).collect();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].target_char_id, 1337);
        assert_eq!(menu_root_count(&mut app), 0);
    }
}
