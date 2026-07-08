//! Minimap widget: top-right HUD showing the current map's minimap BMP, the local
//! player as a rotating direction arrow, and a live `<mapname> <x>, <y>` readout.

use bevy::asset::LoadState;
use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::assets::patterns::minimap_path;
use game_engine::domain::entities::character::components::visual::CharacterDirection;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::world::map::MapData;
use game_engine::utils::coordinates::{world_position_to_spawn_coords, Direction};

use crate::theme;

/// Marks the minimap root.
#[derive(Component)]
pub struct MinimapRoot;

/// Marks the fixed-size viewport that clips the minimap image and marker.
#[derive(Component)]
pub struct MinimapFrame;

/// Marks the `ImageNode` displaying the current map's minimap BMP.
#[derive(Component)]
pub struct MinimapImage;

/// Marks the rotating arrow representing the local player's position and facing.
#[derive(Component)]
pub struct MinimapMarker;

/// Marks the `<mapname> <x>, <y>` coordinate readout text.
#[derive(Component)]
pub struct MinimapCoordText;

/// Caches the current map's minimap dimensions and loaded image handle so the
/// marker/coord systems can detect a map switch by name rather than relying on
/// the transient map entity.
#[derive(Resource, Default)]
pub struct MinimapState {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub handle: Option<Handle<Image>>,
}

/// Maps grid coordinates `(gx, gy)` on a `w`x`h` map to a pixel position
/// `(left, top)` inside a `frame_w`x`frame_h` frame, proportionally, flipping
/// Y since the GAT origin is bottom-left but the UI origin is top-left.
pub fn grid_to_frame_px(
    gx: u16,
    gy: u16,
    w: u32,
    h: u32,
    frame_w: f32,
    frame_h: f32,
) -> (f32, f32) {
    if w == 0 || h == 0 {
        return (0.0, 0.0);
    }

    let frac_x = (gx as f32 / w as f32).clamp(0.0, 1.0);
    let frac_y = (1.0 - gy as f32 / h as f32).clamp(0.0, 1.0);

    (frac_x * frame_w, frac_y * frame_h)
}

/// Maps a facing direction to clockwise degrees for the marker's rotation, with
/// North (up) as the arrow's default orientation.
pub fn direction_to_degrees(facing: Direction) -> f32 {
    ((facing as i32 - 4).rem_euclid(8)) as f32 * 45.0
}

/// Side length in px of the fixed square minimap viewport.
const FRAME_SIZE: f32 = 180.0;

/// Half the marker's side length in px, used to center it on its grid point.
const MARKER_HALF: f32 = 6.0;

/// Builds the minimap under `parent`: a fixed-size map image anchored top-right
/// with the player marker over it and a coordinate readout below. The image stays
/// empty and the marker hidden until the loading/tracking systems run.
pub fn spawn_minimap(commands: &mut Commands, parent: Entity, asset_server: &AssetServer) {
    let font_body = asset_server.load(theme::FONT_BODY);

    let root = commands
        .spawn((
            MinimapRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                right: Val::Px(16.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();

    let frame = commands
        .spawn((
            MinimapFrame,
            Node {
                width: Val::Px(FRAME_SIZE),
                height: Val::Px(FRAME_SIZE),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();

    commands.spawn((
        MinimapImage,
        ImageNode::default(),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(frame),
    ));

    commands.spawn((
        MinimapMarker,
        ImageNode {
            image: asset_server.load(format!("{}arrow.svg", theme::ICON_DIR)),
            color: theme::EMERALD_BRI,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(12.0),
            height: Val::Px(12.0),
            ..default()
        },
        Visibility::Hidden,
        Pickable::IGNORE,
        ChildOf(frame),
    ));

    commands.spawn((
        MinimapCoordText,
        Text::new(""),
        TextFont {
            font: font_body.into(),
            font_size: 11.5.into(),
            ..default()
        },
        TextColor(theme::TEXT_DIM),
        TextShadow {
            offset: Vec2::splat(1.0),
            color: Color::srgba(0.0, 0.0, 0.0, 0.75),
        },
        Pickable::IGNORE,
        ChildOf(root),
    ));
}

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MinimapState>();
        app.add_systems(
            Update,
            (
                sync_minimap_image,
                update_minimap_marker,
                update_minimap_coords,
            )
                .run_if(in_state(GameState::InGame)),
        );
    }
}

/// Detects a map switch by name, loads the new map's minimap BMP into
/// `MinimapState`, and applies it to `MinimapImage`. Each run also polls the
/// current handle's load state: on failure the image and marker are hidden
/// (coords-only fallback), on success the image becomes visible again.
fn sync_minimap_image(
    map_query: Query<&MapData>,
    asset_server: Res<AssetServer>,
    mut minimap_state: ResMut<MinimapState>,
    mut image_query: Query<(&mut ImageNode, &mut Visibility), With<MinimapImage>>,
    mut marker_query: Query<&mut Visibility, (With<MinimapMarker>, Without<MinimapImage>)>,
) {
    let Ok(map) = map_query.single() else {
        return;
    };
    let Ok((mut image_node, mut image_visibility)) = image_query.single_mut() else {
        return;
    };

    if map.name != minimap_state.name {
        let handle: Handle<Image> = asset_server.load(minimap_path(&map.name));
        minimap_state.name = map.name.clone();
        minimap_state.width = map.width;
        minimap_state.height = map.height;
        minimap_state.handle = Some(handle.clone());
        image_node.image = handle;
    }

    let Some(handle) = minimap_state.handle.clone() else {
        return;
    };

    match asset_server.load_state(&handle) {
        LoadState::Failed(_) => {
            *image_visibility = Visibility::Hidden;
            if let Ok(mut marker_visibility) = marker_query.single_mut() {
                *marker_visibility = Visibility::Hidden;
            }
        }
        LoadState::Loaded => {
            *image_visibility = Visibility::Inherited;
        }
        LoadState::Loading | LoadState::NotLoaded => {}
    }
}

/// Marker's mutable node, transform, and visibility, disjoint from `MinimapImage`.
type MarkerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Node,
        &'static mut UiTransform,
        &'static mut Visibility,
    ),
    (With<MinimapMarker>, Without<MinimapImage>),
>;

/// Positions and rotates `MinimapMarker` over the local player's grid position each
/// frame. Stays hidden when there is no local player, no map has been synced yet,
/// or the current map has no minimap BMP (mirrored from `MinimapImage`'s
/// visibility, which `sync_minimap_image` hides on load failure).
fn update_minimap_marker(
    player_query: Query<(&Transform, &CharacterDirection), With<LocalPlayer>>,
    minimap_state: Res<MinimapState>,
    image_visibility_query: Query<&Visibility, (With<MinimapImage>, Without<MinimapMarker>)>,
    mut marker_query: MarkerQuery,
) {
    let Ok((transform, direction)) = player_query.single() else {
        return;
    };
    if minimap_state.width == 0 || minimap_state.height == 0 {
        return;
    }
    let Ok(image_visibility) = image_visibility_query.single() else {
        return;
    };
    if *image_visibility == Visibility::Hidden {
        return;
    }
    let Ok((mut node, mut ui_transform, mut marker_visibility)) = marker_query.single_mut() else {
        return;
    };

    let (gx, gy) = world_position_to_spawn_coords(transform.translation, 0, 0);
    let (left, top) = grid_to_frame_px(
        gx,
        gy,
        minimap_state.width,
        minimap_state.height,
        FRAME_SIZE,
        FRAME_SIZE,
    );

    node.left = Val::Px(left - MARKER_HALF);
    node.top = Val::Px(top - MARKER_HALF);
    ui_transform.rotation = Rot2::degrees(direction_to_degrees(direction.facing));
    *marker_visibility = Visibility::Inherited;
}

/// Writes the `<mapname> <x>, <y>` readout each frame, only touching the `Text`
/// when the value actually changes.
fn update_minimap_coords(
    player_query: Query<&Transform, With<LocalPlayer>>,
    minimap_state: Res<MinimapState>,
    mut coord_query: Query<&mut Text, With<MinimapCoordText>>,
) {
    let Ok(transform) = player_query.single() else {
        return;
    };
    if minimap_state.name.is_empty() {
        return;
    }
    let Ok(mut text) = coord_query.single_mut() else {
        return;
    };

    let (gx, gy) = world_position_to_spawn_coords(transform.translation, 0, 0);
    let value = format!("{} {}, {}", minimap_state.name, gx, gy);
    set_text(&mut text, value);
}

fn set_text(text: &mut Text, value: String) {
    if text.0 != value {
        *text = Text::new(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 180.0;

    #[test]
    fn grid_to_frame_px_top_left_corner() {
        assert_eq!(grid_to_frame_px(0, 100, 100, 100, FRAME, FRAME), (0.0, 0.0));
    }

    #[test]
    fn grid_to_frame_px_bottom_left_corner() {
        assert_eq!(grid_to_frame_px(0, 0, 100, 100, FRAME, FRAME), (0.0, FRAME));
    }

    #[test]
    fn grid_to_frame_px_top_right_corner() {
        assert_eq!(
            grid_to_frame_px(100, 100, 100, 100, FRAME, FRAME),
            (FRAME, 0.0)
        );
    }

    #[test]
    fn grid_to_frame_px_bottom_right_corner() {
        assert_eq!(
            grid_to_frame_px(100, 0, 100, 100, FRAME, FRAME),
            (FRAME, FRAME)
        );
    }

    #[test]
    fn grid_to_frame_px_center() {
        assert_eq!(
            grid_to_frame_px(50, 50, 100, 100, FRAME, FRAME),
            (FRAME / 2.0, FRAME / 2.0)
        );
    }

    #[test]
    fn grid_to_frame_px_clamps_out_of_range_coords() {
        let (left, top) = grid_to_frame_px(500, 500, 100, 100, FRAME, FRAME);
        assert_eq!((left, top), (FRAME, 0.0));
    }

    #[test]
    fn grid_to_frame_px_zero_dimensions_returns_sane_default() {
        assert_eq!(grid_to_frame_px(10, 10, 0, 100, FRAME, FRAME), (0.0, 0.0));
        assert_eq!(grid_to_frame_px(10, 10, 100, 0, FRAME, FRAME), (0.0, 0.0));
    }

    #[test]
    fn direction_to_degrees_maps_all_eight_directions() {
        assert_eq!(direction_to_degrees(Direction::North), 0.0);
        assert_eq!(direction_to_degrees(Direction::NorthEast), 45.0);
        assert_eq!(direction_to_degrees(Direction::East), 90.0);
        assert_eq!(direction_to_degrees(Direction::SouthEast), 135.0);
        assert_eq!(direction_to_degrees(Direction::South), 180.0);
        assert_eq!(direction_to_degrees(Direction::SouthWest), 225.0);
        assert_eq!(direction_to_degrees(Direction::West), 270.0);
        assert_eq!(direction_to_degrees(Direction::NorthWest), 315.0);
    }
}
