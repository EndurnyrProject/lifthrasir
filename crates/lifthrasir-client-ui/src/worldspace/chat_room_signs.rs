//! World-anchored chat room signs: the sign over each NPC or player that runs a room
//! in view, clickable to open the room's card.
//!
//! Like a vending board, a sign keeps a fixed on-screen size and hangs from a
//! world-space point over its owner's head. A sign exists per room the domain
//! reports and is only shown once its owner's entity resolves.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::chat_room::ChatRooms;
use lifthrasir_client::domain::entities::registry::EntityRegistry;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::ignore_picking;
use lifthrasir_ui::widgets::primitives::HoverColors;
use net_contract::state::ZoneSession;

use crate::widgets::chat_room::ChatRoomUi;
use crate::widgets::chat_room::scene::{SIGN_WIDTH, sign_plate};
use crate::widgets::chat_room::view::{self, SignView};
use crate::worldspace::vending_boards::head_anchor;
use crate::worldspace::{WORLD_OVERLAY_Z, WorldCameraFilter, WorldOverlaySystems, viewport_to_ui};

/// Level with the vending boards, just below nameplates (+100).
const SIGN_Z: i32 = WORLD_OVERLAY_Z + 99;

pub struct ChatRoomSignPlugin;

impl Plugin for ChatRoomSignPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                sync_signs.in_set(WorldOverlaySystems::Sync),
                follow_signs.in_set(WorldOverlaySystems::Follow),
            ),
        )
        .add_systems(OnExit(GameState::InGame), despawn_signs);
    }
}

/// The sign of one room; `target` is re-resolved every frame.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct RoomSignLabel {
    pub room_id: u32,
    pub view: SignView,
    /// The local player sits in this room, whose window is already open.
    pub joined: bool,
    pub target: Option<Entity>,
}

/// The sign standing on a zero-height anchor at the head point. The click observer
/// sits on the root, which the plate's events bubble to.
fn sign(view: SignView) -> impl Scene {
    let (ring, ..) = view.tone.colors();
    let colors = HoverColors {
        rest: (ro_dark::TITLE_BAR, ring),
        hover: (ro_dark::BUTTON_HOVER, ring),
    };
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: px(SIGN_WIDTH),
            height: px(0),
        }
        GlobalZIndex(SIGN_Z)
        Visibility::Hidden
        @ignore_picking()
        on(sign_clicked)
        Children [
            Node {
                position_type: PositionType::Absolute,
                bottom: px(0),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
            }
            @ignore_picking()
            Children [
                Hovered
                colors
                @sign_plate(view)
                Pickable
            ]
        ]
    }
}

/// Opens the room's card, or closes it. The room the player sits in has no card: its
/// window is on screen.
fn sign_clicked(
    click: On<PointerClick>,
    labels: Query<&RoomSignLabel>,
    mut ui: ResMut<ChatRoomUi>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let Ok(label) = labels.get(click.entity) else {
        return;
    };
    if label.target.is_none() || label.joined {
        return;
    }
    ui.toggle_card(label.room_id);
}

/// Keeps exactly one sign per room the domain reports. A sign whose look changed is
/// replaced, since the look is drawn into it.
fn sync_signs(
    mut commands: Commands,
    rooms: Res<ChatRooms>,
    session: Res<ZoneSession>,
    registry: Res<EntityRegistry>,
    mut signs: Query<(Entity, &mut RoomSignLabel)>,
    positioned: Query<(), With<GlobalTransform>>,
) {
    let joined = rooms.joined().map(|room| room.sign.room_id);
    for room in rooms.signs().values() {
        let own = !room.owner_is_npc && room.owner_gid == session.char_id;
        let wanted = RoomSignLabel {
            room_id: room.room_id,
            view: view::sign(room, own),
            joined: joined == Some(room.room_id),
            target: registry
                .get_entity(room.owner_gid)
                .filter(|entity| positioned.contains(*entity)),
        };
        let existing = signs
            .iter_mut()
            .find(|(_, label)| label.room_id == room.room_id);
        match existing {
            Some((_, label)) if *label == wanted => {}
            Some((_, mut label)) if label.view == wanted.view => *label = wanted,
            existing => {
                if let Some((entity, _)) = existing {
                    commands.entity(entity).despawn();
                }
                commands
                    .spawn_scene(sign(wanted.view.clone()))
                    .insert(wanted);
            }
        }
    }

    for (entity, label) in &signs {
        if !rooms.signs().contains_key(&label.room_id) {
            commands.entity(entity).despawn();
        }
    }
}

/// Projects each sign onto its owner's head; a sign without a spawned owner, or one
/// outside the viewport, is hidden and not clickable.
fn follow_signs(
    camera: Query<(&Camera, &GlobalTransform), WorldCameraFilter>,
    ui_scale: Res<UiScale>,
    targets: Query<&GlobalTransform>,
    mut signs: Query<(&RoomSignLabel, &mut Node, &mut Visibility)>,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    let camera_up = *camera_transform.up();
    for (label, mut node, mut visibility) in &mut signs {
        let projected = label
            .target
            .and_then(|target| targets.get(target).ok())
            .and_then(|target| {
                let anchor = head_anchor(target.translation(), camera_up);
                camera.world_to_viewport(camera_transform, anchor).ok()
            });
        let Some(screen) = projected else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let position = viewport_to_ui(screen, &ui_scale);
        node.left = Val::Px(position.x - SIGN_WIDTH / 2.0);
        node.top = Val::Px(position.y);
        *visibility = Visibility::Visible;
    }
}

fn despawn_signs(mut commands: Commands, signs: Query<Entity, With<RoomSignLabel>>) {
    for entity in &signs {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;
    use bevy::scene::ScenePlugin;
    use net_contract::dto::{ChatRoomMember, ChatRoomSign};
    use net_contract::events::ChatRoomEvent;

    const ME: u32 = 42;
    const OWNER: u32 = 5;

    fn room(title: &str, member_count: u32) -> ChatRoomSign {
        ChatRoomSign {
            room_id: 9,
            title: title.into(),
            member_count,
            seats: 6,
            public: true,
            owner_gid: OWNER,
            owner_is_npc: false,
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.init_resource::<ChatRooms>()
            .init_resource::<ChatRoomUi>()
            .init_resource::<EntityRegistry>()
            .init_resource::<UiScale>()
            .insert_resource(ZoneSession {
                char_id: ME,
                ..default()
            })
            .add_systems(Update, (sync_signs, follow_signs).chain());
        app
    }

    fn push(app: &mut App, event: ChatRoomEvent) {
        let me = ChatRoomMember {
            char_id: ME,
            name: "Ygor".into(),
        };
        app.world_mut()
            .resource_mut::<ChatRooms>()
            .apply(&event, &me, SystemTime::UNIX_EPOCH);
    }

    fn labels(app: &mut App) -> Vec<(Entity, RoomSignLabel)> {
        let world = app.world_mut();
        world
            .query::<(Entity, &RoomSignLabel)>()
            .iter(world)
            .map(|(entity, label)| (entity, label.clone()))
            .collect()
    }

    fn texts(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        let mut query = world.query::<&Text>();
        query.iter(world).map(|text| text.0.clone()).collect()
    }

    #[test]
    fn a_sign_is_redrawn_when_its_room_changes_and_dropped_with_it() {
        let mut app = app();
        push(
            &mut app,
            ChatRoomEvent::SignShown(room("Selling Elunium", 3)),
        );
        app.update();
        let first = labels(&mut app);
        assert_eq!(first.len(), 1);
        assert!(texts(&mut app).iter().any(|text| text == "3/6"));

        push(
            &mut app,
            ChatRoomEvent::SignShown(room("Selling Elunium", 4)),
        );
        app.update();
        let second = labels(&mut app);
        assert_eq!(second.len(), 1, "one sign per room");
        assert_ne!(first[0].0, second[0].0, "the new count is a new sign");
        let shown = texts(&mut app);
        assert!(shown.iter().any(|text| text == "4/6") && !shown.iter().any(|text| text == "3/6"));

        push(&mut app, ChatRoomEvent::SignRemoved { room_id: 9 });
        app.update();
        assert!(labels(&mut app).is_empty());
        assert!(texts(&mut app).is_empty(), "the plate goes with its sign");
    }

    #[test]
    fn a_sign_resolves_its_owner_without_being_redrawn() {
        let mut app = app();
        push(&mut app, ChatRoomEvent::SignShown(room("Wolfpack", 1)));
        app.update();
        let unresolved = labels(&mut app);
        assert!(unresolved[0].1.target.is_none());

        let owner = app.world_mut().spawn(GlobalTransform::default()).id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(OWNER, owner);
        app.update();
        let resolved = labels(&mut app);
        assert_eq!(resolved[0].0, unresolved[0].0, "the same sign");
        assert_eq!(resolved[0].1.target, Some(owner));
    }

    #[test]
    fn the_plate_blocks_the_world_click_behind_it() {
        let mut app = app();
        push(&mut app, ChatRoomEvent::SignShown(room("Wolfpack", 1)));
        app.update();

        let world = app.world_mut();
        let mut plates = world.query_filtered::<&Pickable, With<HoverColors>>();
        let plate = plates.single(world).expect("one plate");
        assert!(plate.should_block_lower && plate.is_hoverable);
    }
}
