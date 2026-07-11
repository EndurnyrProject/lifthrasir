//! Leader-only right-click "Invite to Party" context menu (BSN + Feathers).
//!
//! A global `Pointer<Click>` observer ([`open_invite_menu`], registered in `PartyPlugin`)
//! fires narrowly: only on a `Secondary` click whose picked root is a remote `Pc`
//! ([`NetworkEntity`] with [`ObjectType::Pc`], `gid != ZoneSession.char_id`) while the
//! local player is the party leader. When any gate fails it does nothing and lets the
//! event pass, so ordinary right-clicks (camera, non-players, the local player, non-leader)
//! are untouched. On a match it stops propagation, despawns any open menu, and spawns a
//! one-item popup at the cursor carrying the target's `gid` in [`ContextMenuTarget`].
//!
//! The popup is a full-screen transparent backdrop (click-away dismiss via
//! [`dismiss_menu`]) holding a small glass card at the cursor; the card stops click
//! propagation so interacting inside it never triggers the backdrop dismiss. Its single
//! "Invite to Party" `@FeathersButton` writes [`PartyInviteRequested`] with the stored
//! `target_char_id` and closes the menu.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::{ButtonVariant, FeathersButton};
use bevy_feathers::theme::ThemedText;
use game_engine::domain::entities::components::NetworkEntity;
use game_engine::domain::entities::types::ObjectType;
use game_engine::domain::party::PartyState;
use net_contract::commands::PartyInviteRequested;
use net_contract::state::ZoneSession;

use crate::theme;

/// Below the create (`MAX - 4`) and system (`MAX - 2`) dialogs: a transient popup never
/// needs to stack over a modal, and layering it under them keeps a disconnect notice on top.
const MENU_Z: i32 = i32::MAX - 5;

const MENU_WIDTH: f32 = 168.0;

/// The menu root (full-screen backdrop). The invite/dismiss observers resolve the open
/// menu by this marker, and it carries [`ContextMenuTarget`].
#[derive(Component, Default, Clone)]
pub struct PartyContextMenuRoot;

/// The clicked player's `char_id` (`NetworkEntity.gid`), stored on the menu root so the
/// invite button knows whom to invite.
#[derive(Component, Default, Clone)]
pub struct ContextMenuTarget(pub u32);

/// Resolve the picked child to its owning root: the body billboard is a `ChildOf` the
/// `NetworkEntity` root, so a picked child resolves to its parent; a rootless pick is its
/// own root. Mirrors `entities::picking::pick_root`.
fn pick_root(child: Entity, child_of: &Query<&ChildOf>) -> Entity {
    child_of.get(child).map(|c| c.parent()).unwrap_or(child)
}

/// Pure gate: show the invite menu only for a `Secondary` click on a remote `Pc` while the
/// local player is the party leader. Any failing gate returns `false` (event passes through).
pub fn should_show_invite_menu(
    button: PointerButton,
    net: Option<&NetworkEntity>,
    is_local: bool,
    is_leader: bool,
) -> bool {
    button == PointerButton::Secondary
        && !is_local
        && is_leader
        && matches!(net, Some(net) if net.object_type == ObjectType::Pc)
}

/// Global observer: pops the leader-only invite menu on a qualifying right-click. Narrow by
/// construction (see [`should_show_invite_menu`]); when it fires it stops propagation so the
/// event never reaches ancestor entities, keeping a single menu regardless of bubbling.
pub fn open_invite_menu(
    mut click: On<Pointer<Click>>,
    child_of: Query<&ChildOf>,
    nets: Query<&NetworkEntity>,
    session: Res<ZoneSession>,
    party: Res<PartyState>,
    existing: Query<Entity, With<PartyContextMenuRoot>>,
    mut commands: Commands,
) {
    let root = pick_root(click.entity, &child_of);
    let net = nets.get(root).ok();
    let is_local = net.is_some_and(|net| net.gid == session.char_id);
    let is_leader = party.is_leader(session.char_id);

    if !should_show_invite_menu(click.event.button, net, is_local, is_leader) {
        return;
    }

    let net = net.expect("should_show_invite_menu guarantees Some(Pc)");
    click.propagate(false);

    for menu in existing.iter() {
        commands.entity(menu).despawn();
    }

    let cursor = click.pointer_location.position;
    commands.spawn_scene(context_menu(cursor, net.gid));
}

/// The popup: a transparent, click-eating full-screen backdrop (dismiss on click-away)
/// holding the cursor-anchored menu card.
// NOTE: not clamped to the viewport; a right-click near the screen edge can clip the card.
// Add edge clamping (needs window size) if it becomes a problem.
fn context_menu(cursor: Vec2, target: u32) -> impl Scene {
    bsn! {
        PartyContextMenuRoot
        ContextMenuTarget({target})
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

/// The glass card anchored at the cursor. Stops click propagation so clicks inside it never
/// bubble to the backdrop's dismiss observer.
fn card(cursor: Vec2) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: {px(cursor.x)},
            top: {px(cursor.y)},
            width: px(MENU_WIDTH),
            padding: {UiRect::all(px(6))},
            flex_direction: FlexDirection::Column,
            border: px(1),
            border_radius: BorderRadius::all(px(10)),
        }
        BackgroundColor({theme::GLASS})
        BorderColor::all(theme::STROKE)
        Pickable
        on(|mut click: On<Pointer<Click>>| click.propagate(false))
        Children [ invite_button() ]
    }
}

/// The single "Invite to Party" item.
fn invite_button() -> impl Scene {
    bsn! {
        @FeathersButton {
            @caption: bsn! {
                (
                    Text("Invite to Party")
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
        on(on_invite)
    }
}

fn on_invite(
    _: On<Activate>,
    menu: Query<(Entity, &ContextMenuTarget), With<PartyContextMenuRoot>>,
    mut writer: MessageWriter<PartyInviteRequested>,
    mut commands: Commands,
) {
    let Ok((root, target)) = menu.single() else {
        return;
    };
    writer.write(PartyInviteRequested {
        target_char_id: target.0,
        target_name: String::new(),
    });
    commands.entity(root).despawn();
}

fn dismiss_menu(
    _: On<Pointer<Click>>,
    menu: Query<Entity, With<PartyContextMenuRoot>>,
    mut commands: Commands,
) {
    if let Ok(root) = menu.single() {
        commands.entity(root).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pc(gid: u32) -> NetworkEntity {
        NetworkEntity::new(gid, gid, ObjectType::Pc)
    }

    #[test]
    fn shows_for_leader_right_clicking_remote_pc() {
        assert!(should_show_invite_menu(
            PointerButton::Secondary,
            Some(&pc(42)),
            false,
            true,
        ));
    }

    #[test]
    fn hidden_for_primary_button() {
        assert!(!should_show_invite_menu(
            PointerButton::Primary,
            Some(&pc(42)),
            false,
            true,
        ));
    }

    #[test]
    fn hidden_when_not_leader() {
        assert!(!should_show_invite_menu(
            PointerButton::Secondary,
            Some(&pc(42)),
            false,
            false,
        ));
    }

    #[test]
    fn hidden_for_local_player() {
        assert!(!should_show_invite_menu(
            PointerButton::Secondary,
            Some(&pc(42)),
            true,
            true,
        ));
    }

    #[test]
    fn hidden_when_no_network_entity() {
        assert!(!should_show_invite_menu(
            PointerButton::Secondary,
            None,
            false,
            true,
        ));
    }

    #[test]
    fn hidden_for_non_pc_object() {
        let mob = NetworkEntity::new(1, 1, ObjectType::Mob);
        assert!(!should_show_invite_menu(
            PointerButton::Secondary,
            Some(&mob),
            false,
            true,
        ));
    }

    #[test]
    fn invite_writes_command_with_stored_gid_and_despawns() {
        let mut app = App::new();
        app.add_message::<PartyInviteRequested>();

        let root = app
            .world_mut()
            .spawn((PartyContextMenuRoot, ContextMenuTarget(1337)))
            .id();
        let button = app.world_mut().spawn_empty().observe(on_invite).id();

        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        let messages = app.world().resource::<Messages<PartyInviteRequested>>();
        let mut cursor = messages.get_cursor();
        let written: Vec<_> = cursor.read(messages).collect();
        assert_eq!(written.len(), 1, "exactly one invite command");
        assert_eq!(written[0].target_char_id, 1337);
        assert_eq!(written[0].target_name, "");
        assert!(
            app.world().get_entity(root).is_err(),
            "inviting despawns the menu"
        );
    }
}
