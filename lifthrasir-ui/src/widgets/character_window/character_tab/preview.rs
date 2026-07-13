//! Live, rotatable character preview for the Console's Character tab.
//!
//! Ported from the old `equipment_window/preview.rs`. The render-to-texture pipeline
//! is unchanged: the local player's real sprite + equipped headgear is rendered into
//! the tab's preview frame via an off-screen orthographic `Camera3d`, isolated from
//! the world camera by a dedicated [`RenderLayers`].
//!
//! Type ownership: the UI-local markers/resources are renamed so the old window can be
//! deleted whole in the integration task — [`ConsolePreviewCharacter`],
//! [`ConsolePreviewState`], [`ConsoleLocalHeadgear`], [`CharPreviewFrame`]. But the
//! camera + billboard markers [`EquipmentPreviewCamera`] and [`PreviewBillboard`] are
//! **game-engine DOMAIN types**, not UI types: ~8 engine systems exclude
//! `EquipmentPreviewCamera` when resolving "the" world `Camera3d`, and
//! `preview_billboard_rotation_system` orients `PreviewBillboard` entities at it.
//! Inventing a new camera marker would make the world `billboard_rotation_system`'s
//! `single()` see two cameras and bail, breaking world billboard facing. So these two
//! are reused exactly like `CharacterDirection` / `EquipmentSet`.
//!
//! Activation differs from the old window: the camera + character are spawned only
//! while [`CharacterWindowState`] is `open` on the Character tab, and despawned when
//! leaving that tab / closing / on `OnExit(InGame)`. The render-target `Image` and its
//! `ImageNode` binding persist across tab switches (never recreated, camera never
//! double-spawned).

use std::collections::HashMap;

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{
    ClearColorConfig, OrthographicProjection, Projection, RenderTarget, ScalingMode,
};
use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};
use game_engine::domain::entities::billboard::{
    Billboard, EquipmentPreviewCamera, PreviewBillboard,
};
use game_engine::domain::entities::character::components::visual::{
    CharacterDirection, CharacterSprite, Direction,
};
use game_engine::domain::entities::character::components::{
    CharacterAppearance, CharacterData, EquipmentItem, EquipmentSet, EquipmentSlot,
};
use game_engine::domain::entities::character::SpawnCharacterSpriteEvent;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::entities::sprite_rendering::EquipmentChangeEvent;

use crate::screens::character_preview::create_render_target;
use crate::theme;
use crate::theme::feathers_theme::{TOKEN_PANEL_BG, TOKEN_TEXT_DIM, TOKEN_WINDOW_BORDER};
use crate::widgets::chrome::glyph_icon;

use super::super::{CharacterTab, CharacterWindowState};

/// Render-target dimensions; 2x the preview frame's box so the character supersamples
/// down crisply.
const PREVIEW_W: u32 = 180;
const PREVIEW_H: u32 = 240;
/// World-space vertical extent the orthographic camera frames.
const VIEWPORT_HEIGHT: f32 = 42.0;
/// Vertical aim offset so the camera frames the body, not the feet at the origin.
const LOOK_AT_Y: f32 = -8.0;
/// Camera offset mirroring the in-world RO isometric tilt, so this camera's rotation
/// matches the world follow camera's — that match is what lets the engine drive the
/// preview character's head/headgear from the world camera.
const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, -150.0, -150.0);
/// Where the preview rig lives, far from the play area.
const PREVIEW_ORIGIN: Vec3 = Vec3::new(0.0, 100_000.0, 0.0);
/// Dedicated render layer isolating the preview camera + billboards from the world.
const PREVIEW_LAYER: usize = 1;

/// Marker on the spawned preview character root.
#[derive(Component)]
pub struct ConsolePreviewCharacter;

/// Center preview-frame mount; the `ImageNode` binding rides this.
#[derive(Component, Default, Clone)]
pub struct CharPreviewFrame;

/// Holds the preview render target so it is created once per in-game session.
#[derive(Resource, Default)]
pub struct ConsolePreviewState {
    target: Option<Handle<Image>>,
}

/// The local player's currently equipped headgear (`slot -> view id`), accumulated
/// from the equipment change stream so the preview can mirror it on spawn.
#[derive(Resource, Default)]
pub struct ConsoleLocalHeadgear(HashMap<EquipmentSlot, u16>);

/// Whether the preview rig should be alive: the Console open on the Character tab.
fn preview_active(state: &CharacterWindowState) -> bool {
    state.open && state.tab == CharacterTab::Character
}

// ---------------------------------------------------------------------------
// Scene: preview frame + rotate row.
// ---------------------------------------------------------------------------

/// The center column: the render-target frame over a Rotate row.
pub fn preview_frame() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(6),
            padding: {UiRect::horizontal(px(4))},
        }
        crate::widgets::chrome::ignore_picking()
        Children [
            (
                CharPreviewFrame
                Node {
                    width: px(165),
                    height: px(220),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: px(1),
                    border_radius: BorderRadius::all(px(8)),
                }
                ThemeBackgroundColor({TOKEN_PANEL_BG})
                ThemeBorderColor({TOKEN_WINDOW_BORDER})
            ),
            rotate_row(),
        ]
    }
}

fn rotate_row() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
        }
        crate::widgets::chrome::ignore_picking()
        Children [
            (
                @FeathersButton { @caption: bsn! { glyph_icon("rotl", 12.0, theme::TEXT_DIM) } }
                Node { width: px(24), height: px(20) }
                on(on_rotate_left)
            ),
            (
                Text({"Rotate".to_string()})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(10.0)},
                }
                ThemeTextColor({TOKEN_TEXT_DIM})
                crate::widgets::chrome::ignore_picking()
            ),
            (
                @FeathersButton { @caption: bsn! { glyph_icon("rotr", 12.0, theme::TEXT_DIM) } }
                Node { width: px(24), height: px(20) }
                on(on_rotate_right)
            ),
        ]
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

/// Drives the whole preview rig off [`CharacterWindowState`]: while the Console is open
/// on the Character tab, ensure the render target + `ImageNode` (once per session), the
/// camera, and the preview character all exist; otherwise despawn the camera +
/// character while keeping the `Image` so a later re-open reuses it.
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
            Transform::from_translation(look_at + CAMERA_OFFSET).looking_at(look_at, Vec3::NEG_Y),
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
    fn preview_active_only_when_open_on_character_tab() {
        assert!(preview_active(&CharacterWindowState {
            open: true,
            tab: CharacterTab::Character,
        }));
        assert!(!preview_active(&CharacterWindowState {
            open: false,
            tab: CharacterTab::Character,
        }));
        assert!(!preview_active(&CharacterWindowState {
            open: true,
            tab: CharacterTab::Bag,
        }));
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
