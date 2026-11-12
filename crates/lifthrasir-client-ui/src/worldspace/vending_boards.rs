//! World-anchored vending boards: a shop sign over each visible vendor, clickable to
//! open its shop. Hovering one shows the whole title, which the sign itself cuts off.
//!
//! The sign keeps a fixed on-screen size for legibility, but its anchor is a
//! world-space point over the vendor's head, so it stays on the head at any
//! camera zoom instead of drifting like a fixed pixel lift would. A board exists
//! per vendor identity and is kept while the vendor is out of view or its entity
//! has not spawned yet, so the cached title and shop id stay available; it is
//! only rendered and clickable once the target resolves.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::components::EntityName;
use lifthrasir_client::domain::entities::registry::EntityRegistry;
use lifthrasir_client::domain::vending::{VendingIntent, VendingState};
use lifthrasir_runtime::rendering::layers::SPRITE_BASE_Y_OFFSET;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::primitives::HoverColors;

use crate::screens::shared::{shop_sign, sign_title};
use crate::worldspace::{WorldCameraFilter, viewport_to_ui};

/// Widest a sign gets; a longer title is cut off here and shown whole on hover.
const BOARD_WIDTH: f32 = 220.0;
const NOTCH_SIZE: f32 = 8.0;
const TOOLTIP_WIDTH: f32 = 250.0;
/// How far above the head point the tooltip's bottom edge sits: over the sign.
const TOOLTIP_LIFT: f32 = 42.0;
/// World units from the sprite quads' base up to just over the head, measured
/// along the billboard's up axis (the camera's up). NOTE: tuned for a seated
/// vendor; adjust live via BRP if the notch sits off the head.
const BOARD_HEAD_RISE: f32 = 10.0;
/// Just below nameplates (+100); see [`super::WORLD_OVERLAY_Z`].
const BOARD_Z: i32 = super::WORLD_OVERLAY_Z + 99;

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
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct VendingBoardLabel {
    pub vendor_unit_id: u64,
    pub shop_id: String,
    pub title: String,
    /// The local player's own shop, which the sign marks with a frost halo.
    pub own: bool,
    pub target: Option<Entity>,
}

/// The clickable sign plate.
#[derive(Component, Default, Clone)]
pub struct VendingBoardPlate;

/// The full-title tooltip of a hovered sign.
#[derive(Component, Default, Clone)]
pub struct VendingBoardTooltip;

/// The sign hanging from a zero-height anchor at the head point: the plate, then the
/// notch under it. The observers sit on the root, which the plate's events bubble to.
fn board(title: String, own: bool) -> impl Scene {
    let glow = if own {
        ro_dark::FROST_GLOW.with_alpha(0.45)
    } else {
        Color::NONE
    };
    let colors = HoverColors {
        rest: (ro_dark::TITLE_BAR, ro_dark::BRONZE),
        hover: (ro_dark::BUTTON_HOVER, ro_dark::BRONZE_HIGHLIGHT),
    };
    // A square turned 45 degrees points its bottom-right corner down, so its right
    // and bottom borders draw the notch outline under the plate.
    let tilt = UiTransform::from_rotation(Rot2::degrees(45.0));
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: px(BOARD_WIDTH),
            height: px(0),
        }
        GlobalZIndex(BOARD_Z)
        Visibility::Hidden
        @ignore_picking()
        on(board_clicked)
        on(show_board_title)
        on(hide_board_title)
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
                VendingBoardPlate
                Hovered
                colors
                @shop_sign(sign_title(title), BOARD_WIDTH, glow)
                Pickable
                --
                Node {
                    width: px(NOTCH_SIZE),
                    height: px(NOTCH_SIZE),
                    margin: {UiRect::top(px(-NOTCH_SIZE / 2.0 - 1.0))},
                    border: {UiRect::new(px(0), px(1), px(0), px(1))},
                }
                tilt
                BackgroundColor(ro_dark::TITLE_BAR)
                BorderColor::all(ro_dark::BRONZE)
                @ignore_picking()
            ]
        ]
    }
}

/// The tooltip over a hovered sign: who runs the shop when that is known, the whole
/// title, and what a click does.
fn board_tooltip(vendor: Option<String>, title: String, own: bool) -> impl Scene {
    let vendor: Vec<_> = vendor
        .into_iter()
        .map(|name| chrome_text(name, 13.0, ro_dark::HERO))
        .collect();
    let hint = if own {
        "Click to manage your shop"
    } else {
        "Click to browse"
    };
    bsn! {
        VendingBoardTooltip
        Node {
            position_type: PositionType::Absolute,
            left: px((BOARD_WIDTH - TOOLTIP_WIDTH) / 2.0),
            bottom: px(TOOLTIP_LIFT),
            width: px(TOOLTIP_WIDTH),
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            padding: {UiRect::axes(px(12), px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(ro_dark::TOOLTIP_BODY)
        BorderColor::all(ro_dark::BUTTON_STROKE)
        @ignore_picking()
        Children [
            {vendor}
            --
            @chrome_text(title, 12.0, ro_dark::BODY)
            --
            Node { margin: {UiRect::top(px(6))}, padding: {UiRect::top(px(6))}, border: {UiRect::top(px(1))} }
            BorderColor::all(ro_dark::SLOT_STROKE)
            @ignore_picking()
            Children [ @chrome_text(hint.into(), 11.0, ro_dark::MUTED) ]
        ]
    }
}

/// Opens the board's shop: the owner enters their seller window, anyone else
/// browses the displayed shop identity.
fn board_clicked(
    click: On<PointerClick>,
    labels: Query<&VendingBoardLabel>,
    mut intents: MessageWriter<VendingIntent>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let Ok(label) = labels.get(click.entity) else {
        return;
    };
    if let Some(intent) = board_intent(label) {
        intents.write(intent);
    }
}

fn show_board_title(
    over: On<PointerOver>,
    labels: Query<&VendingBoardLabel>,
    names: Query<&EntityName>,
    mut commands: Commands,
) {
    let Ok(label) = labels.get(over.entity) else {
        return;
    };
    let vendor = label
        .target
        .and_then(|target| names.get(target).ok())
        .map(|name| name.name.clone());
    commands
        .spawn_scene(board_tooltip(vendor, label.title.clone(), label.own))
        .insert(ChildOf(over.entity));
}

fn hide_board_title(
    _: On<PointerOut>,
    tooltips: Query<Entity, With<VendingBoardTooltip>>,
    mut commands: Commands,
) {
    for tooltip in &tooltips {
        commands.entity(tooltip).despawn();
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

/// The world point the sign hangs from: the sprite base lifted along the
/// billboard's up axis, which is the camera's up because billboards copy the
/// camera rotation.
pub fn head_anchor(feet: Vec3, camera_up: Vec3) -> Vec3 {
    feet + Vec3::Y * SPRITE_BASE_Y_OFFSET + camera_up * BOARD_HEAD_RISE
}

/// Keeps exactly one board per vendor the domain reports. A board whose shop, title
/// or ownership changed is replaced, since all three are drawn into the sign, and
/// boards that are no longer reported are dropped.
fn sync_vending_boards(
    mut commands: Commands,
    state: Res<VendingState>,
    registry: Res<EntityRegistry>,
    mut boards: Query<(Entity, &mut VendingBoardLabel)>,
    positioned: Query<(), With<GlobalTransform>>,
) {
    let own_unit_id = registry.local_player_account_id().map(u64::from);
    for board in state.boards().values() {
        let wanted = VendingBoardLabel {
            vendor_unit_id: board.vendor_unit_id,
            shop_id: board.shop_id.clone(),
            title: board.title.clone(),
            own: own_unit_id == Some(board.vendor_unit_id),
            target: u32::try_from(board.vendor_unit_id)
                .ok()
                .and_then(|unit_id| registry.get_entity(unit_id))
                .filter(|entity| positioned.contains(*entity)),
        };
        let existing = boards
            .iter_mut()
            .find(|(_, label)| label.vendor_unit_id == board.vendor_unit_id);
        match existing {
            Some((_, label)) if *label == wanted => {}
            Some((_, mut label)) if same_sign(&label, &wanted) => label.target = wanted.target,
            existing => {
                if let Some((entity, _)) = existing {
                    commands.entity(entity).despawn();
                }
                commands
                    .spawn_scene(self::board(wanted.title.clone(), wanted.own))
                    .insert(wanted);
            }
        }
    }

    for (entity, label) in &boards {
        if !state.boards().contains_key(&label.vendor_unit_id) {
            commands.entity(entity).despawn();
        }
    }
}

/// Whether two labels draw the same sign, whatever they point at.
fn same_sign(a: &VendingBoardLabel, b: &VendingBoardLabel) -> bool {
    (&a.shop_id, &a.title, a.own) == (&b.shop_id, &b.title, b.own)
}

/// Projects each visible board onto its vendor's head; a board without a
/// spawned target, or one outside the viewport, is hidden and not clickable.
fn follow_vending_boards(
    camera: Query<(&Camera, &GlobalTransform), WorldCameraFilter>,
    ui_scale: Res<UiScale>,
    targets: Query<&GlobalTransform>,
    mut boards: Query<(&VendingBoardLabel, &mut Node, &mut Visibility)>,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    let camera_up = *camera_transform.up();
    for (label, mut node, mut visibility) in &mut boards {
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
        node.left = Val::Px(position.x - BOARD_WIDTH / 2.0);
        node.top = Val::Px(position.y);
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
    use bevy::scene::ScenePlugin;
    use net_contract::dto::VendingBoard;
    use net_contract::events::{VendingIngress, VendingIngressPayload};
    use net_contract::state::ZoneSessionGeneration;

    const VENDOR: u64 = 5;
    const OWN: u32 = 42;
    const LONG_TITLE: &str = "S> MVP cards, Bloody Branch, whisper me for bulk";

    fn board(shop_id: &str, title: &str) -> VendingBoard {
        VendingBoard {
            vendor_unit_id: VENDOR,
            shop_id: shop_id.into(),
            title: title.into(),
        }
    }

    fn app(own_char_id: Option<u32>) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.insert_resource(VendingState::default())
            .insert_resource(EntityRegistry::default())
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

    fn texts(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        let mut query = world.query::<&Text>();
        query.iter(world).map(|text| text.0.clone()).collect()
    }

    fn label(own: bool, target: Option<Entity>) -> VendingBoardLabel {
        VendingBoardLabel {
            vendor_unit_id: VENDOR,
            shop_id: "shop-1".into(),
            title: "Potions".into(),
            own,
            target,
        }
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
        assert!(all_hidden(&mut app), "an unresolved board is never shown");
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
        app.update();

        let shown = boards(&mut app);
        assert_eq!(shown.len(), 1, "resolving the vendor keeps the same board");
        assert_eq!(shown[0].1.target, Some(target), "the vendor resolves");

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
    fn a_replaced_shop_replaces_the_sign_and_browses_the_new_one() {
        let mut app = app(Some(OWN));
        show(&mut app, board("shop-1", "Potions"));
        app.update();
        assert_eq!(boards(&mut app).len(), 1);
        assert!(texts(&mut app).iter().any(|title| title == "Potions"));

        show(&mut app, board("shop-2", "Scrolls"));
        app.update();
        let second = boards(&mut app);
        assert_eq!(second.len(), 1, "one board per vendor");
        assert_eq!(second[0].1.shop_id, "shop-2");
        let titles = texts(&mut app);
        assert!(titles.iter().any(|title| title == "Scrolls"));
        assert!(!titles.iter().any(|title| title == "Potions"));
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
        assert!(texts(&mut app).is_empty(), "the sign goes with its board");

        show(&mut app, board("shop-3", "Potions"));
        app.update();
        assert_eq!(boards(&mut app).len(), 1);
        push(&mut app, VendingIngressPayload::MapChanging);
        app.update();
        assert!(boards(&mut app).is_empty(), "a map change clears boards");
    }

    #[test]
    fn own_and_remote_boards_open_different_flows() {
        assert_eq!(board_intent(&label(false, None)), None);

        let target = Some(Entity::PLACEHOLDER);
        assert_eq!(
            board_intent(&label(false, target)),
            Some(VendingIntent::Browse {
                vendor_unit_id: VENDOR,
                shop_id: "shop-1".into(),
            })
        );
        assert_eq!(
            board_intent(&label(true, target)),
            Some(VendingIntent::EnterSeller)
        );
    }

    #[test]
    fn the_plate_blocks_the_world_click_behind_it() {
        let mut app = app(Some(OWN));
        show(&mut app, board("shop-1", "Potions"));
        app.update();

        let world = app.world_mut();
        let mut plates = world.query_filtered::<&Pickable, With<VendingBoardPlate>>();
        let plate = plates.single(world).expect("one plate");
        assert!(plate.should_block_lower && plate.is_hoverable);
    }

    #[test]
    fn the_sign_hangs_over_the_head_along_the_billboard_up_axis() {
        let feet = Vec3::new(10.0, 2.0, -4.0);
        let level = head_anchor(feet, Vec3::Y);
        assert_eq!(
            level,
            feet + Vec3::Y * (SPRITE_BASE_Y_OFFSET + BOARD_HEAD_RISE)
        );

        // A tilted camera tilts the billboard, so the head point tilts with it.
        let tilted_up = Vec3::new(0.0, 0.7, -0.7).normalize();
        let tilted = head_anchor(feet, tilted_up);
        assert!((tilted - (feet + Vec3::Y * SPRITE_BASE_Y_OFFSET)).dot(tilted_up) > 0.0);
        assert!(tilted.z < feet.z, "the head point leans with the camera");
    }

    #[test]
    fn a_board_becomes_the_owner_sign_once_the_local_account_is_known() {
        let mut app = app(None);
        show(&mut app, board("shop-1", "Potions"));
        app.update();
        assert!(!boards(&mut app)[0].1.own);

        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .set_local_player(Entity::PLACEHOLDER, VENDOR as u32);
        app.update();
        let after = boards(&mut app);
        assert_eq!(after.len(), 1, "still one board per vendor");
        assert!(after[0].1.own);
    }

    #[test]
    fn hovering_a_sign_shows_the_whole_title_until_the_pointer_leaves() {
        let mut app = app(Some(OWN));
        let board = app
            .world_mut()
            .spawn(VendingBoardLabel {
                title: LONG_TITLE.into(),
                ..label(false, None)
            })
            .observe(show_board_title)
            .observe(hide_board_title)
            .id();
        let window = app.world_mut().spawn_empty().id();
        let pointer = |window| {
            use bevy::camera::NormalizedRenderTarget;
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
            )
        };
        let hit = bevy::picking::backend::HitData::new(board, 0.0, None, None);

        app.world_mut().trigger(PointerOver {
            entity: board,
            pointer: pointer(window),
            hit: hit.clone(),
        });
        app.world_mut().flush();
        let shown = texts(&mut app);
        assert!(shown.iter().any(|text| text == LONG_TITLE), "{shown:?}");
        assert!(shown.iter().any(|text| text == "Click to browse"));

        app.world_mut().trigger(PointerOut {
            entity: board,
            pointer: pointer(window),
            hit,
        });
        app.world_mut().flush();
        assert!(texts(&mut app).is_empty());
    }
}
