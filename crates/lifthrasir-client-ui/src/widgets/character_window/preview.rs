//! Live, rotatable character preview on the Character window's stage.
//!
//! The local player's real sprite + equipped headgear is rendered into the stage via
//! an off-screen orthographic `Camera3d`, isolated from the world camera by a
//! dedicated [`RenderLayers`].
//!
//! The camera + billboard markers [`EquipmentPreviewCamera`] and [`PreviewBillboard`]
//! are **game-engine DOMAIN types**, not UI types: ~8 engine systems exclude
//! `EquipmentPreviewCamera` when resolving "the" world `Camera3d`, and
//! `preview_billboard_rotation_system` orients `PreviewBillboard` entities at it.
//! Inventing a new camera marker would make the world `billboard_rotation_system`'s
//! `single()` see two cameras and bail, breaking world billboard facing.
//!
//! The camera + character are spawned only while [`CharacterWindowState`] is `open`,
//! and despawned on close and on `OnExit(InGame)`. The render-target `Image` and its
//! `ImageNode` binding persist across reopens (never recreated, camera never
//! double-spawned).

use std::collections::HashMap;

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{
    ClearColorConfig, OrthographicProjection, Projection, RenderTarget, ScalingMode,
};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::character::SpawnCharacterSpriteEvent;
use lifthrasir_client::domain::entities::character::events::forward_character_sprite_events;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_runtime::actors::sprites::legacy::{CharacterDirection, CharacterSprite, Direction};
use lifthrasir_runtime::actors::sprites::systems::events::EquipmentChangeEvent;
use lifthrasir_runtime::rendering::billboard::{
    Billboard, EquipmentPreviewCamera, PreviewBillboard,
};
use {
    lifthrasir_client::domain::entities::character::components::{
        CharacterAppearance, CharacterData, EquipmentItem, EquipmentSet,
    },
    lifthrasir_runtime::actors::EquipmentSlot,
};

use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};

use crate::screens::character_preview::create_render_target;
use crate::screens::shared::small_button;
use crate::widgets::character_window::CharacterWindowState;

/// The stage the hero stands on, between the two slot columns.
const STAGE: Vec2 = Vec2::new(200.0, 350.0);
/// Displayed size of the render target. The camera frames a standing hero to fill it
/// with the feet near its bottom edge.
const FIGURE: Vec2 = Vec2::new(186.0, 248.0);
/// Where the pedestal's centre sits below the stage's top edge.
const PEDESTAL_Y: f32 = 288.0;
/// How far above the figure's bottom edge its feet are drawn.
const FOOT_INSET: f32 = 3.0;
/// Render-target dimensions; 2x [`FIGURE`] so the character supersamples down crisply.
const PREVIEW_W: u32 = 372;
const PREVIEW_H: u32 = 496;
/// World-space vertical extent the orthographic camera frames.
const VIEWPORT_HEIGHT: f32 = 42.0;
/// Vertical aim offset so the camera frames the body, not the feet at the origin.
const LOOK_AT_Y: f32 = 8.0;
/// Camera offset mirroring the in-world RO isometric tilt, so this camera's rotation
/// matches the world follow camera's — that match is what lets the engine drive the
/// preview character's head/headgear from the world camera.
const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 150.0, 150.0);
/// Where the preview rig lives, far from the play area.
const PREVIEW_ORIGIN: Vec3 = Vec3::new(0.0, 100_000.0, 0.0);
/// Dedicated render layer isolating the preview camera + billboards from the world.
const PREVIEW_LAYER: usize = 1;

/// Marker on the spawned preview character root.
#[derive(Component)]
pub struct ConsolePreviewCharacter;

/// The figure's mount on the stage; the `ImageNode` binding rides this.
#[derive(Component, Default, Clone)]
pub struct CharPreviewFrame;

/// The text under the hero naming the way they face.
#[derive(Component, Default, Clone)]
pub struct FacingLabel;

/// Holds the preview render target so it is created once per in-game session.
#[derive(Resource, Default)]
pub struct ConsolePreviewState {
    target: Option<Handle<Image>>,
}

/// The local player's currently equipped headgear (`slot -> view id`), accumulated
/// from the equipment change stream so the preview can mirror it on spawn.
#[derive(Resource, Default)]
pub struct ConsoleLocalHeadgear(HashMap<EquipmentSlot, u16>);

/// Register the preview's resources and systems into `CharacterWindowPlugin`.
pub fn register(app: &mut App) {
    app.init_resource::<ConsolePreviewState>()
        .init_resource::<ConsoleLocalHeadgear>()
        .add_systems(
            Update,
            (
                cache_local_headgear,
                manage_console_preview.after(forward_character_sprite_events),
                forward_preview_headgear,
                tag_preview_billboards,
                sync_facing_label,
            )
                .run_if(in_state(GameState::InGame)),
        )
        .add_systems(OnExit(GameState::InGame), cleanup_preview);
}

/// Whether the preview rig should be alive: the window is open.
fn preview_active(state: &CharacterWindowState) -> bool {
    state.open
}

// ---------------------------------------------------------------------------
// Scene: the stage with its pedestal and rotate row.
// ---------------------------------------------------------------------------

/// The lit stage: a frost glow from above, the pedestal rings, the hero and the
/// rotate controls along the bottom.
pub fn stage() -> impl Scene {
    let backdrop = BackgroundGradient::from(LinearGradient::to_bottom(vec![
        ColorStop::new(ro_dark::INSET, percent(0)),
        ColorStop::new(ro_dark::BAR_TROUGH, percent(100)),
    ]));
    let figure_top = PEDESTAL_Y + FOOT_INSET - FIGURE.y;
    bsn! {
        Node {
            width: px(STAGE.x),
            height: px(STAGE.y),
            flex_shrink: 0.0,
            position_type: PositionType::Relative,
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
            overflow: {Overflow::clip()},
        }
        backdrop
        BorderColor::all(ro_dark::SLOT_STROKE)
        @ignore_picking()
        Children [
            @glow(Vec2::new(224.0, 270.0), 105.0, 0.13)
            --
            @glow(Vec2::new(172.0, 44.0), PEDESTAL_Y, 0.35)
            --
            @ring(Vec2::new(140.0, 30.0), 0.55)
            --
            @ring(Vec2::new(116.0, 22.0), 0.4)
            --
            CharPreviewFrame
            @centered(FIGURE, figure_top)
            --
            @rotate_row()
        ]
    }
}

/// A node `size` large, centred on the stage's width with its top edge `top` px down.
fn centered(size: Vec2, top: f32) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px((STAGE.x - size.x) / 2.0),
            top: px(top),
            width: px(size.x),
            height: px(size.y),
        }
        @ignore_picking()
    }
}

/// A frost light fading out from its centre, `centre_y` px down the stage.
fn glow(size: Vec2, centre_y: f32, alpha: f32) -> impl Scene {
    let light = BackgroundGradient::from(RadialGradient {
        shape: RadialGradientShape::Ellipse(percent(50), percent(50)),
        stops: vec![
            ColorStop::new(ro_dark::FROST_GLOW.with_alpha(alpha), percent(0)),
            ColorStop::new(ro_dark::FROST_GLOW.with_alpha(0.0), percent(100)),
        ],
        ..default()
    });
    bsn! {
        @centered(size, centre_y - size.y / 2.0)
        light
    }
}

/// One of the pedestal's rings, drawn as the outline of a flat ellipse.
fn ring(size: Vec2, alpha: f32) -> impl Scene {
    let stroke = ro_dark::FROST.with_alpha(alpha);
    bsn! {
        @centered(size, PEDESTAL_Y - size.y / 2.0)
        Node { border: px(1), border_radius: BorderRadius::MAX_ELLIPTICAL }
        BorderColor::all(stroke)
    }
}

fn rotate_row() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            bottom: px(12),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(10),
        }
        @ignore_picking()
        Children [
            @small_button("chevl", 24.0)
            on(on_rotate_left)
            --
            FacingLabel
            @chrome_text(facing_label(Direction::South).into(), 11.0, ro_dark::MUTED)
            Node { width: px(80) }
            TextLayout { justify: Justify::Center, linebreak: LineBreak::NoWrap }
            --
            @small_button("chevr", 24.0)
            on(on_rotate_right)
        ]
    }
}

fn facing_label(facing: Direction) -> &'static str {
    match facing {
        Direction::South => "Facing south",
        Direction::SouthWest => "Facing southwest",
        Direction::West => "Facing west",
        Direction::NorthWest => "Facing northwest",
        Direction::North => "Facing north",
        Direction::NorthEast => "Facing northeast",
        Direction::East => "Facing east",
        Direction::SouthEast => "Facing southeast",
    }
}

/// Names the way the preview hero faces whenever it turns or is respawned.
fn sync_facing_label(
    preview: Query<
        &CharacterDirection,
        (With<ConsolePreviewCharacter>, Changed<CharacterDirection>),
    >,
    mut labels: Query<&mut Text, With<FacingLabel>>,
) {
    let Ok(direction) = preview.single() else {
        return;
    };
    for mut text in &mut labels {
        text.0 = facing_label(direction.facing).to_string();
    }
}

// ---------------------------------------------------------------------------
// Lifecycle.
// ---------------------------------------------------------------------------

fn equipment_set_from(cache: &HashMap<EquipmentSlot, u16>) -> EquipmentSet {
    let item = |slot: EquipmentSlot| {
        cache.get(&slot).map(|&sprite_id| EquipmentItem {
            item_id: sprite_id as u32,
            sprite_id,
            refinement: 0,
            enchantments: vec![],
            options: vec![],
        })
    };
    EquipmentSet {
        head_top: item(EquipmentSlot::HeadTop),
        head_mid: item(EquipmentSlot::HeadMid),
        head_bottom: item(EquipmentSlot::HeadBottom),
        ..EquipmentSet::default()
    }
}

/// Track the local player's headgear from the equipment change stream. Only the local
/// player emits these (self-targeted), and only for headgear slots.
pub fn cache_local_headgear(
    mut changes: MessageReader<EquipmentChangeEvent>,
    local: Query<Entity, With<LocalPlayer>>,
    mut cache: ResMut<ConsoleLocalHeadgear>,
) {
    let Ok(local) = local.single() else {
        return;
    };
    for change in changes.read() {
        if change.character != local {
            continue;
        }
        match change.view_id {
            Some(view_id) => {
                cache.0.insert(change.slot, view_id);
            }
            None => {
                cache.0.remove(&change.slot);
            }
        }
    }
}

/// Drives the whole preview rig off [`CharacterWindowState`]: while the window is
/// open, ensure the render target + `ImageNode` (once per session), the camera, and
/// the preview character all exist; otherwise despawn the camera + character while
/// keeping the `Image` so a later re-open reuses it.
#[allow(clippy::too_many_arguments)]
pub fn manage_console_preview(
    mut commands: Commands,
    state: Res<CharacterWindowState>,
    mut images: ResMut<Assets<Image>>,
    mut preview_state: ResMut<ConsolePreviewState>,
    mut spawn_events: MessageWriter<SpawnCharacterSpriteEvent>,
    frame: Query<Entity, With<CharPreviewFrame>>,
    cameras: Query<Entity, With<EquipmentPreviewCamera>>,
    characters: Query<Entity, With<ConsolePreviewCharacter>>,
    local: Query<(&CharacterData, &CharacterAppearance), With<LocalPlayer>>,
    cache: Res<ConsoleLocalHeadgear>,
) {
    if !preview_active(&state) {
        for entity in &characters {
            commands.entity(entity).despawn();
        }
        for entity in &cameras {
            commands.entity(entity).despawn();
        }
        return;
    }

    let Ok((data, appearance)) = local.single() else {
        return;
    };

    let target = match &preview_state.target {
        Some(handle) => handle.clone(),
        None => {
            let handle = images.add(create_render_target(PREVIEW_W, PREVIEW_H));
            if let Ok(frame) = frame.single() {
                commands.spawn((
                    ImageNode::new(handle.clone()),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                    ChildOf(frame),
                ));
            }
            preview_state.target = Some(handle.clone());
            handle
        }
    };

    if cameras.is_empty() {
        let look_at = PREVIEW_ORIGIN + Vec3::new(0.0, LOOK_AT_Y, 0.0);
        commands.spawn((
            Camera3d::default(),
            Camera {
                clear_color: ClearColorConfig::Custom(Color::NONE),
                order: -1,
                ..default()
            },
            RenderTarget::Image(target.clone().into()),
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: VIEWPORT_HEIGHT,
                },
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_translation(look_at + CAMERA_OFFSET).looking_at(look_at, Vec3::Y),
            RenderLayers::layer(PREVIEW_LAYER),
            EquipmentPreviewCamera,
            Name::new("ConsolePreviewCamera"),
        ));
    }

    if characters.is_empty() {
        let entity = commands
            .spawn((
                data.clone(),
                appearance.clone(),
                equipment_set_from(&cache.0),
                CharacterSprite::default(),
                CharacterDirection::default(),
                Transform::from_translation(PREVIEW_ORIGIN),
                Visibility::default(),
                ConsolePreviewCharacter,
                Name::new("ConsolePreviewCharacter"),
            ))
            .id();

        spawn_events.write(SpawnCharacterSpriteEvent {
            character_entity: entity,
            spawn_position: PREVIEW_ORIGIN,
        });
    }
}

/// Forward the local player's live headgear changes onto the preview character so
/// equipping / unequipping updates the preview in place (no respawn).
pub fn forward_preview_headgear(
    mut messages: ParamSet<(
        MessageReader<EquipmentChangeEvent>,
        MessageWriter<EquipmentChangeEvent>,
    )>,
    local: Query<Entity, With<LocalPlayer>>,
    preview: Query<Entity, With<ConsolePreviewCharacter>>,
) {
    let (Ok(local), Ok(preview)) = (local.single(), preview.single()) else {
        return;
    };
    let forwarded: Vec<EquipmentChangeEvent> = messages
        .p0()
        .read()
        .filter(|change| change.character == local)
        .map(|change| EquipmentChangeEvent {
            character: preview,
            slot: change.slot,
            view_id: change.view_id,
        })
        .collect();
    let mut outgoing = messages.p1();
    for change in forwarded {
        outgoing.write(change);
    }
}

/// Put freshly spawned preview billboards (body / head / headgear children) onto the
/// preview render layer and tag them so the preview camera faces them. Engine sprite
/// children spawn over several frames, so this runs every frame.
pub fn tag_preview_billboards(
    mut commands: Commands,
    preview: Query<&Children, With<ConsolePreviewCharacter>>,
    billboards: Query<(), (With<Billboard>, Without<PreviewBillboard>)>,
) {
    for children in preview.iter() {
        for child in children.iter() {
            if billboards.contains(child) {
                commands
                    .entity(child)
                    .insert((RenderLayers::layer(PREVIEW_LAYER), PreviewBillboard));
            }
        }
    }
}

/// Despawn the preview character + camera when leaving the game so no second camera or
/// sprite leaks into the next session, and drop the render target so the next session
/// rebuilds it. The frame's `ImageNode` rides the HUD root, despawned with the HUD.
pub fn cleanup_preview(
    mut commands: Commands,
    mut state: ResMut<ConsolePreviewState>,
    mut cache: ResMut<ConsoleLocalHeadgear>,
    characters: Query<Entity, With<ConsolePreviewCharacter>>,
    cameras: Query<Entity, With<EquipmentPreviewCamera>>,
) {
    for entity in &characters {
        commands.entity(entity).despawn();
    }
    for entity in &cameras {
        commands.entity(entity).despawn();
    }
    state.target = None;
    cache.0.clear();
}

/// Step the preview character's 8-way facing; RO sprites are directional, so cycling
/// the facing cycles the displayed frame (no transform rotation).
fn step_facing(facing: Direction, delta: i8) -> Direction {
    Direction::from_u8((facing as i8 + delta).rem_euclid(8) as u8)
}

fn on_rotate_left(
    _: On<Activate>,
    mut preview: Query<&mut CharacterDirection, With<ConsolePreviewCharacter>>,
) {
    if let Ok(mut direction) = preview.single_mut() {
        direction.facing = step_facing(direction.facing, -1);
    }
}

fn on_rotate_right(
    _: On<Activate>,
    mut preview: Query<&mut CharacterDirection, With<ConsolePreviewCharacter>>,
) {
    if let Ok(mut direction) = preview.single_mut() {
        direction.facing = step_facing(direction.facing, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_active_only_while_the_window_is_open() {
        assert!(preview_active(&CharacterWindowState { open: true }));
        assert!(!preview_active(&CharacterWindowState { open: false }));
    }

    #[test]
    fn the_figure_stands_on_the_pedestal_inside_the_stage() {
        let figure_bottom = PEDESTAL_Y + FOOT_INSET;
        assert!(figure_bottom - FIGURE.y >= 0.0);
        assert!(figure_bottom <= STAGE.y);
        assert_eq!(PREVIEW_W as f32 / PREVIEW_H as f32, FIGURE.x / FIGURE.y);
    }

    #[test]
    fn rotate_right_wraps_past_the_last_direction() {
        assert_eq!(step_facing(Direction::SouthEast, 1), Direction::South);
    }

    #[test]
    fn rotate_left_wraps_below_the_first_direction() {
        assert_eq!(step_facing(Direction::South, -1), Direction::SouthEast);
    }

    #[test]
    fn rotate_steps_through_all_eight_then_returns() {
        let mut facing = Direction::South;
        for _ in 0..8 {
            facing = step_facing(facing, 1);
        }
        assert_eq!(facing, Direction::South);
    }

    #[test]
    fn equipment_set_mirrors_cached_headgear() {
        let mut cache = HashMap::new();
        cache.insert(EquipmentSlot::HeadTop, 42u16);
        cache.insert(EquipmentSlot::HeadBottom, 7u16);

        let set = equipment_set_from(&cache);

        assert_eq!(set.head_top.map(|i| i.sprite_id), Some(42));
        assert_eq!(set.head_bottom.map(|i| i.sprite_id), Some(7));
        assert!(set.head_mid.is_none());
    }
}
