//! World-anchored vending boards: a screen-projected title plate over each
//! visible vendor, clickable to open its shop.
//!
//! The projection follows `floor_item_labels.rs` (project the actor's
//! `GlobalTransform` each frame, hide when the target has no viewport position).
//! A board exists per vendor identity and is kept while the vendor is out of
//! view or its entity has not spawned yet, so the cached title and shop id stay
//! available; it is only rendered and clickable once the target resolves.

use bevy::prelude::*;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::registry::EntityRegistry;
use lifthrasir_client::domain::vending::{VendingIntent, VendingState};

use crate::worldspace::{WorldCameraFilter, WorldspaceFont, viewport_to_ui};

/// Bounded board width; a longer title is clipped here and shown in full by the
/// seller or buyer window, which the board opens.
const BOARD_WIDTH: f32 = 168.0;
const BOARD_FONT_SIZE: f32 = 13.0;
/// Pixels above the vendor's projected origin, so the plate clears the sprite.
const BOARD_LIFT: f32 = 74.0;
/// Above the world camera, below the fade overlay and cursor.
const BOARD_Z: i32 = 99;

pub struct VendingBoardPlugin;

impl Plugin for VendingBoardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (sync_vending_boards, follow_vending_boards)
                .chain()
                .run_if(in_state(GameState::InGame)),
        )
        .add_systems(OnExit(GameState::InGame), despawn_all_boards);
    }
}

/// A board for one vendor identity; `target` is re-resolved every frame.
#[derive(Component, Debug, Clone)]
pub struct VendingBoardLabel {
    pub vendor_unit_id: u64,
    pub shop_id: String,
    pub own: bool,
    pub target: Option<Entity>,
    pub title: Entity,
}

/// The clickable plate; clicking it opens the shop.
#[derive(Component, Default, Clone)]
pub struct VendingBoardPlate;

/// The title text node, updated in place when the displayed shop changes.
#[derive(Component, Default, Clone)]
pub struct VendingBoardTitle;

fn spawn_board(
    commands: &mut Commands,
    font: &WorldspaceFont,
    board: &net_contract::dto::VendingBoard,
    own: bool,
    target: Option<Entity>,
) -> Entity {
    let (fill, border, label) = if own {
        (
            lifthrasir_ui::theme::GLASS,
            lifthrasir_ui::theme::GOLD_FAINT,
            format!("My Shop: {}", board.title),
        )
    } else {
        (
            lifthrasir_ui::theme::GLASS_2,
            lifthrasir_ui::theme::STROKE_STRONG,
            board.title.clone(),
        )
    };
    let title = commands
        .spawn((
            VendingBoardTitle,
            Text::new(label),
            TextFont {
                font: font.0.clone().into(),
                font_size: BOARD_FONT_SIZE.into(),
                ..default()
            },
            TextColor(lifthrasir_ui::theme::TEXT),
            TextLayout::no_wrap(),
            Pickable::IGNORE,
        ))
        .id();
    let plate = commands
        .spawn((
            VendingBoardPlate,
            Node {
                max_width: Val::Px(BOARD_WIDTH),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(fill),
            BorderColor::all(border),
            Pickable::default(),
        ))
        .observe(board_clicked)
        .add_child(title)
        .id();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(BOARD_WIDTH),
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(BOARD_Z),
            Visibility::Hidden,
            Pickable::IGNORE,
            VendingBoardLabel {
                vendor_unit_id: board.vendor_unit_id,
                shop_id: board.shop_id.clone(),
                own,
                target,
                title,
            },
        ))
        .add_child(plate)
        .id()
}

/// Opens the board's shop: the owner enters their seller window, anyone else
/// browses the displayed shop identity.
fn board_clicked(
    click: On<PointerClick>,
    plates: Query<&ChildOf, With<VendingBoardPlate>>,
    labels: Query<&VendingBoardLabel>,
    mut intents: MessageWriter<VendingIntent>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let Ok(child_of) = plates.get(click.entity) else {
        return;
    };
    let Ok(label) = labels.get(child_of.parent()) else {
        return;
    };
    if let Some(intent) = board_intent(label) {
        intents.write(intent);
    }
}

/// What a click on this board asks for; an unresolved vendor asks for nothing.
pub fn board_intent(label: &VendingBoardLabel) -> Option<VendingIntent> {
    label.target?;
    if label.own {
        return Some(VendingIntent::EnterSeller);
    }
    Some(VendingIntent::Browse {
        vendor_unit_id: label.vendor_unit_id,
        shop_id: label.shop_id.clone(),
    })
}

/// Keeps exactly one board per vendor the domain reports, updating a replaced
/// shop identity in place and dropping boards that are no longer reported.
fn sync_vending_boards(
    mut commands: Commands,
    state: Res<VendingState>,
    registry: Res<EntityRegistry>,
    font: Res<WorldspaceFont>,
    mut boards: Query<(Entity, &mut VendingBoardLabel)>,
    mut titles: Query<&mut Text, With<VendingBoardTitle>>,
    positioned: Query<(), With<GlobalTransform>>,
) {
    let own_unit_id = registry.local_player_account_id().map(u64::from);
    for board in state.boards().values() {
        let own = own_unit_id == Some(board.vendor_unit_id);
        let target = u32::try_from(board.vendor_unit_id)
            .ok()
            .and_then(|unit_id| registry.get_entity(unit_id))
            .filter(|entity| positioned.contains(*entity));
        let existing = boards
            .iter_mut()
            .find(|(_, label)| label.vendor_unit_id == board.vendor_unit_id);
        match existing {
            Some((_, mut label)) => {
                label.target = target;
                if label.shop_id != board.shop_id {
                    label.shop_id.clone_from(&board.shop_id);
                    label.own = own;
                    if let Ok(mut text) = titles.get_mut(label.title) {
                        text.0 = if own {
                            format!("My Shop: {}", board.title)
                        } else {
                            board.title.clone()
                        };
                    }
                }
            }
            None => {
                spawn_board(&mut commands, &font, board, own, target);
            }
        }
    }

    for (entity, label) in &boards {
        if !state.boards().contains_key(&label.vendor_unit_id) {
            commands.entity(entity).despawn();
        }
    }
}

/// Projects each visible board onto its vendor; a board without a spawned
/// target, or one outside the viewport, is hidden and not clickable.
fn follow_vending_boards(
    camera: Query<(&Camera, &GlobalTransform), WorldCameraFilter>,
    ui_scale: Res<UiScale>,
    targets: Query<&GlobalTransform>,
    mut boards: Query<(&VendingBoardLabel, &mut Node, &mut Visibility)>,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    for (label, mut node, mut visibility) in &mut boards {
        let projected = label
            .target
            .and_then(|target| targets.get(target).ok())
            .and_then(|target| {
                camera
                    .world_to_viewport(camera_transform, target.translation())
                    .ok()
            });
        let Some(screen) = projected else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let position = viewport_to_ui(screen, &ui_scale);
        node.left = Val::Px(position.x - BOARD_WIDTH / 2.0);
        node.top = Val::Px(position.y - BOARD_LIFT);
        *visibility = Visibility::Visible;
    }
}

fn despawn_all_boards(mut commands: Commands, boards: Query<Entity, With<VendingBoardLabel>>) {
    for entity in &boards {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::VendingBoard;
    use net_contract::events::{VendingIngress, VendingIngressPayload};
    use net_contract::state::ZoneSessionGeneration;

    const VENDOR: u64 = 5;
    const OWN: u32 = 42;

    fn board(shop_id: &str, title: &str) -> VendingBoard {
        VendingBoard {
            vendor_unit_id: VENDOR,
            shop_id: shop_id.into(),
            title: title.into(),
        }
    }

    fn app(own_char_id: Option<u32>) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(VendingState::default())
            .insert_resource(EntityRegistry::default())
            .insert_resource(WorldspaceFont(Handle::default()))
            .init_resource::<UiScale>()
            .add_message::<VendingIntent>();
        if let Some(char_id) = own_char_id {
            let entity = app.world_mut().spawn_empty().id();
            app.world_mut()
                .resource_mut::<EntityRegistry>()
                .set_local_player(entity, char_id);
        }
        app.add_systems(Update, (sync_vending_boards, follow_vending_boards).chain());
        app
    }

    fn show(app: &mut App, board: VendingBoard) {
        push(app, VendingIngressPayload::BoardShown(board));
    }

    fn push(app: &mut App, payload: VendingIngressPayload) {
        app.world_mut()
            .resource_mut::<VendingState>()
            .apply(&VendingIngress {
                generation: ZoneSessionGeneration::default(),
                payload,
            });
    }

    fn all_hidden(app: &mut App) -> bool {
        let world = app.world_mut();
        world
            .query::<&Visibility>()
            .iter(world)
            .all(|visibility| *visibility != Visibility::Visible)
    }

    fn boards(app: &mut App) -> Vec<(Entity, VendingBoardLabel)> {
        let world = app.world_mut();
        world
            .query::<(Entity, &VendingBoardLabel)>()
            .iter(world)
            .map(|(entity, label)| (entity, label.clone()))
            .collect()
    }

    #[test]
    fn a_board_without_its_vendor_is_cached_but_not_clickable() {
        let mut app = app(Some(OWN));
        show(&mut app, board("shop-1", "Potions"));
        app.update();

        let boards = boards(&mut app);
        assert_eq!(boards.len(), 1, "the board is cached");
        assert!(boards[0].1.target.is_none());
        assert_eq!(board_intent(&boards[0].1), None);
        let mut visibility = app.world_mut().query::<&Visibility>();
        assert!(
            visibility
                .iter(app.world())
                .all(|visibility| *visibility != Visibility::Visible),
            "an unresolved board is never shown"
        );
    }

    #[test]
    fn a_board_hides_when_its_target_or_projection_is_gone() {
        let mut app = app(Some(OWN));
        let target = app.world_mut().spawn(GlobalTransform::default()).id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(VENDOR as u32, target);
        show(&mut app, board("shop-1", "Potions"));
        app.update();

        assert_eq!(
            boards(&mut app)[0].1.target,
            Some(target),
            "the vendor resolves"
        );

        // A camera with no viewport cannot project, so the board stays hidden.
        app.world_mut().spawn(Camera3d::default());
        app.update();
        assert!(all_hidden(&mut app), "no projection means no board");

        app.world_mut().entity_mut(target).despawn();
        app.update();
        assert!(all_hidden(&mut app), "a despawned vendor hides its board");
        assert_eq!(board_intent(&boards(&mut app)[0].1), None);
    }

    #[test]
    fn replaced_shop_ids_update_the_existing_board_and_browse_the_display() {
        let mut app = app(Some(OWN));
        show(&mut app, board("shop-1", "Potions"));
        app.update();
        let first = boards(&mut app);
        assert_eq!(first.len(), 1);

        show(&mut app, board("shop-2", "Scrolls"));
        app.update();
        let second = boards(&mut app);
        assert_eq!(second.len(), 1, "one board per vendor");
        assert_eq!(second[0].0, first[0].0, "the board is reused");
        assert_eq!(second[0].1.shop_id, "shop-2");
        let mut texts = app.world_mut().query::<&Text>();
        let titles: Vec<String> = texts.iter(app.world()).map(|text| text.0.clone()).collect();
        assert!(titles.iter().any(|title| title == "Scrolls"));
    }

    #[test]
    fn a_removed_or_reset_board_is_despawned() {
        let mut app = app(Some(OWN));
        show(&mut app, board("shop-1", "Potions"));
        app.update();
        assert_eq!(boards(&mut app).len(), 1);

        push(
            &mut app,
            VendingIngressPayload::BoardRemoved {
                unit_id: VENDOR,
                shop_id: "shop-1".into(),
                reason: net_contract::dto::VendingClosureReason::UserClosed,
            },
        );
        app.update();
        assert!(boards(&mut app).is_empty());

        show(&mut app, board("shop-3", "Potions"));
        app.update();
        assert_eq!(boards(&mut app).len(), 1);
        push(&mut app, VendingIngressPayload::MapChanging);
        app.update();
        assert!(boards(&mut app).is_empty(), "a map change clears boards");
    }

    #[test]
    fn own_and_remote_boards_open_different_flows() {
        let mut label = VendingBoardLabel {
            vendor_unit_id: VENDOR,
            shop_id: "shop-1".into(),
            own: false,
            target: None,
            title: Entity::PLACEHOLDER,
        };
        assert_eq!(board_intent(&label), None);

        let target = Entity::PLACEHOLDER;
        label.target = Some(target);
        assert_eq!(
            board_intent(&label),
            Some(VendingIntent::Browse {
                vendor_unit_id: VENDOR,
                shop_id: "shop-1".into(),
            })
        );

        label.own = true;
        assert_eq!(board_intent(&label), Some(VendingIntent::EnterSeller));
    }

    #[test]
    fn the_owner_board_is_labelled_and_every_plate_blocks_lower_clicks() {
        let mut app = app(Some(OWN));
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .set_local_player(Entity::PLACEHOLDER, VENDOR as u32);
        show(&mut app, board("shop-1", "Potions"));
        app.update();

        let boards = boards(&mut app);
        assert!(boards[0].1.own);
        let mut plates = app.world_mut().query::<&Pickable>();
        assert!(
            plates
                .iter(app.world())
                .any(|pickable| pickable.should_block_lower),
            "the clickable plate blocks the world click behind it"
        );
    }
}
