//! World-anchored overlays: name plates, bars, bubbles and floating damage numbers.
//!
//! These are screen-projected `bevy_ui` nodes (not `bevy_lunex` worldspace UI): each
//! frame an overlay's `left`/`top` is set from
//! `Camera::world_to_viewport(entity_position)`. This keeps the classic RO
//! always-on-top look, adds no dependency, and needs no changes to the in-game
//! `Camera3d`.
//!
//! An overlay that tracks a unit carries a [`WorldAnchor`] on a root built by
//! [`hung_root`] or [`raised_root`]; one system places them all.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::Font;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::EntityRegistry;
use lifthrasir_client::domain::input::LockedTarget;
use lifthrasir_runtime::rendering::billboard::EquipmentPreviewCamera;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::ignore_picking;

pub mod chat_bubbles;
pub mod damage_numbers;
#[cfg(feature = "dev")]
pub mod debug_gizmos;
pub mod floor_item_labels;
pub mod mob_health_bars;
pub mod nameplates;
pub mod progress_bar;
pub mod scribble_labels;
pub mod skill_cast_labels;
pub mod target_ring;
pub mod vending_boards;

/// Query filter for the in-game 3D camera that worldspace overlays project
/// against, excluding the equipment-window preview camera.
pub type WorldCameraFilter = (With<Camera3d>, Without<EquipmentPreviewCamera>);

/// Stacking base for every world-anchored overlay: below the HUD windows
/// (default 0) and NPC cut-ins (-1), so a window covering an overlay hides it
/// and takes its clicks. Each overlay adds a small offset to keep their order.
/// UI always draws over the 3D world, whatever the value.
pub const WORLD_OVERLAY_Z: i32 = -1000;

/// The fill of the plate behind a hovered or targeted unit's lines.
const PLATE_FILL: Color = Color::srgba(0.047, 0.055, 0.071, 0.62);

/// Shared font handle for nameplates and damage numbers, loaded once at startup.
#[derive(Resource)]
pub struct WorldspaceFont(pub Handle<Font>);

/// Order of the anchored overlays' systems within a frame.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum WorldOverlaySystems {
    /// Overlays are spawned, updated and despawned.
    Sync,
    /// Every anchored overlay is placed on its unit.
    Follow,
}

/// Pins an overlay root to a unit: the root is `width` wide, centred on the unit's
/// projected origin, with its top edge `drop` pixels below it (negative lifts it).
/// The overlay is despawned once the unit is gone.
///
/// NOTE: `drop` is a fixed screen offset, not zoom-scaled. Tune it live over BRP if
/// an overlay drifts off its sprite.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct WorldAnchor {
    pub target: Entity,
    pub width: f32,
    pub drop: f32,
}

/// The unit the hero has locked as attack target.
#[derive(SystemParam)]
pub struct LockedUnit<'w> {
    locked: Res<'w, LockedTarget>,
    registry: Res<'w, EntityRegistry>,
}

impl LockedUnit<'_> {
    pub fn entity(&self) -> Option<Entity> {
        self.locked
            .gid
            .and_then(|gid| self.registry.get_entity(gid))
    }
}

/// Maps a `Camera::world_to_viewport` result (logical viewport pixels) into the
/// `bevy_ui` layout space. The UI lays out at `window.scale_factor() * UiScale`,
/// so its `Val::Px` coordinates are the logical viewport divided by the UI
/// scale; without this, projected overlays drift off their sprites at any
/// scale other than 100%.
pub fn viewport_to_ui(viewport: Vec2, ui_scale: &UiScale) -> Vec2 {
    viewport / ui_scale.0
}

/// The root of an overlay that hangs below its anchor point: a `width`-wide strip
/// that centres its children. Hidden until its [`WorldAnchor`] places it.
pub fn hung_root(width: f32, z: i32) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: px(width),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
        }
        GlobalZIndex({z})
        Visibility::Hidden
        @ignore_picking()
    }
}

/// The root of an overlay that stands on its anchor point and grows upward, so
/// content of any height keeps its bottom edge on the point. `content` is centred
/// in a `width`-wide strip. Hidden until its [`WorldAnchor`] places it.
pub fn raised_root(width: f32, z: i32, content: impl Scene) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: px(width),
            height: px(0),
        }
        GlobalZIndex({z})
        Visibility::Hidden
        @ignore_picking()
        Children [
            Node {
                position_type: PositionType::Absolute,
                bottom: px(0),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
            }
            @ignore_picking()
            Children [ @content ]
        ]
    }
}

/// The frame around a unit's lines: clear at rest, a dark bronze-ringed plate when
/// `backed`. It keeps its padding either way so the lines do not move on hover.
pub fn plate_frame(backed: bool) -> impl Scene {
    let (fill, ring) = if backed {
        (PLATE_FILL, ro_dark::DIVIDER.with_alpha(0.55))
    } else {
        (Color::NONE, Color::NONE)
    };
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(1),
            padding: {UiRect::new(px(8), px(8), px(3), px(4))},
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(fill)
        BorderColor::all(ring)
        @ignore_picking()
    }
}

/// A small bar over a unit: a dark trough of `size` ringed in `ring`. `fill` marks
/// and colours the full-width fill, which the owner then narrows.
pub fn world_bar(size: Vec2, ring: Color, fill: impl Scene) -> impl Scene {
    bsn! {
        Node {
            width: px(size.x + 2.0),
            height: px(size.y + 2.0),
            flex_shrink: 0.0,
            border: px(1),
            border_radius: BorderRadius::all(px(3)),
        }
        BackgroundColor({Color::BLACK.with_alpha(0.7)})
        BorderColor::all(ring)
        @ignore_picking()
        Children [
            Node {
                width: percent(100),
                height: percent(100),
                border_radius: BorderRadius::all(px(2)),
            }
            @ignore_picking()
            @fill
        ]
    }
}

/// `amount` of `limit` as a fill width, clamped to the bar.
pub fn fill_width(amount: u64, limit: u64) -> Val {
    Val::Percent((amount as f32 / limit.max(1) as f32).clamp(0.0, 1.0) * 100.0)
}

pub struct WorldspaceUiPlugin;

impl Plugin for WorldspaceUiPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (WorldOverlaySystems::Sync, WorldOverlaySystems::Follow)
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(Startup, load_font);
        app.add_systems(Update, follow_anchors.in_set(WorldOverlaySystems::Follow));
        app.add_systems(OnExit(GameState::InGame), despawn_anchored);
        app.add_plugins((
            nameplates::NameplatePlugin,
            target_ring::TargetRingPlugin,
            damage_numbers::DamageNumberPlugin,
            floor_item_labels::FloorItemLabelPlugin,
            mob_health_bars::MobHealthBarPlugin,
            skill_cast_labels::SkillCastLabelPlugin,
            chat_bubbles::ChatBubblePlugin,
            progress_bar::NpcProgressBarPlugin,
            vending_boards::VendingBoardPlugin,
            scribble_labels::ScribbleLabelPlugin,
        ));
    }
}

fn load_font(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(WorldspaceFont(
        asset_server.load(lifthrasir_ui::theme::FONT_BODY),
    ));
}

/// Places every anchored overlay on its unit, hides the ones projected off screen
/// and drops the ones whose unit is gone.
fn follow_anchors(
    camera: Query<(&Camera, &GlobalTransform), WorldCameraFilter>,
    targets: Query<&GlobalTransform>,
    ui_scale: Res<UiScale>,
    mut overlays: Query<(Entity, &WorldAnchor, &mut Node, &mut Visibility)>,
    mut commands: Commands,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    for (entity, anchor, mut node, mut visibility) in &mut overlays {
        let Ok(target) = targets.get(anchor.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        let Ok(screen) = camera.world_to_viewport(camera_transform, target.translation()) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let position = viewport_to_ui(screen, &ui_scale);
        node.left = Val::Px(position.x - anchor.width / 2.0);
        node.top = Val::Px(position.y + anchor.drop);
        *visibility = Visibility::Visible;
    }
}

fn despawn_anchored(mut commands: Commands, overlays: Query<Entity, With<WorldAnchor>>) {
    for entity in &overlays {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_width_is_clamped() {
        assert_eq!(fill_width(50, 100), Val::Percent(50.0));
        assert_eq!(fill_width(0, 100), Val::Percent(0.0));
        assert_eq!(fill_width(200, 100), Val::Percent(100.0));
        assert_eq!(fill_width(5, 0), Val::Percent(100.0));
    }

    /// The overlays order themselves against each other and the guild systems; a
    /// cycle there only shows when the schedule is first built.
    #[test]
    fn the_overlay_schedule_builds_beside_the_guild_systems() {
        use bevy::state::app::StatesPlugin;
        use lifthrasir_client::domain::guild::GuildPlugin;
        use net_contract::events::{GuildIngress, ZoneDisconnected};
        use net_contract::state::ZoneSessionGeneration;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
            .init_asset::<Font>()
            .init_state::<GameState>()
            .add_message::<GuildIngress>()
            .add_message::<ZoneDisconnected>()
            .insert_resource(ZoneSessionGeneration(1))
            .add_plugins((GuildPlugin, WorldspaceUiPlugin));

        app.update();
    }

    #[test]
    fn an_overlay_whose_unit_is_gone_is_dropped() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).init_resource::<UiScale>();
        app.world_mut().spawn((
            Camera3d::default(),
            Camera::default(),
            GlobalTransform::default(),
        ));
        let unit = app.world_mut().spawn(GlobalTransform::default()).id();
        let anchor = |target| WorldAnchor {
            target,
            width: 10.0,
            drop: 0.0,
        };
        let kept = app.world_mut().spawn((anchor(unit), Node::default())).id();
        let orphan = app
            .world_mut()
            .spawn((anchor(Entity::PLACEHOLDER), Node::default()))
            .id();
        app.add_systems(Update, follow_anchors);

        app.update();

        assert!(app.world().get_entity(kept).is_ok());
        assert!(app.world().get_entity(orphan).is_err());
    }
}
