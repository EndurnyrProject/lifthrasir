//! Off-screen 3D "diorama" that renders animated character previews for the
//! character-selection cards.
//!
//! `bevy_ui` cannot render live SPR/ACT sprites directly, and the old PNG round-trip
//! (`sprite_png`) only existed for the deleted Tauri webview. Instead, the real
//! in-world billboard pipeline renders every occupied slot's character — laid out
//! in a single row — into ONE off-screen render-target `Image` via a single
//! orthographic `Camera3d`. Each card then shows its character by cropping that
//! shared target to the character's column (`ImageNode.rect`).
//!
//! A single `Camera3d` is deliberate: `billboard_rotation_system` bails when more
//! than one `Camera3d` exists, and during `CharacterSelection` there is no in-game
//! camera, so this preview camera is the only one and billboards face it correctly.
//! The `SpriteRenderingSystems` set is ungated for `CharacterSelection`
//! (see `game_engine::domain::system_sets::in_game_or_character_select`) so the
//! previews animate through the exact same path as in-world characters.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::{
    ClearColorConfig, OrthographicProjection, Projection, RenderTarget, ScalingMode,
};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use game_engine::core::state::GameState;
use game_engine::domain::character::events::{
    CharacterInfoWithJobName, CharacterListReceivedEvent,
};
use game_engine::domain::entities::character::components::visual::{
    CharacterDirection, CharacterSprite,
};
use game_engine::domain::entities::character::components::CharacterInfo;
use game_engine::domain::entities::character::events::forward_character_sprite_events;
use game_engine::domain::entities::character::SpawnCharacterSpriteEvent;

/// Pixel width of one character column in the shared render target. Also the
/// display width of each card's preview image.
pub const COLUMN_PX: u32 = 144;
/// Pixel height of the render target, and the display height of each card preview.
pub const ROW_PX: u32 = 224;
/// World-space vertical extent the orthographic preview camera shows. Tuned (via
/// live BRP) so a standing character fills the column; horizontal extent derives
/// from the target aspect ratio (`viewport_height * width / height`).
const PREVIEW_VIEWPORT_HEIGHT: f32 = 42.0;
/// Vertical aim offset (world Y, down-positive) so the camera frames the character's
/// body rather than its feet at the origin. Tuned to center the sprite in the card.
const LOOK_AT_Y: f32 = -8.0;
/// Camera offset from the row it frames, mirroring the in-world RO isometric tilt
/// (`CameraFollowSettings::default().offset`). Magnitude is irrelevant for an
/// orthographic projection (only `scaling_mode` sets the size); only the direction
/// — and thus the viewing angle — matters. Y is down in this world.
const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, -150.0, -150.0);

/// Marker for the spawned preview character entities (despawned on rebuild/exit).
#[derive(Component)]
pub struct PreviewCharacter;

/// Marker for the single off-screen preview camera.
#[derive(Component)]
pub struct PreviewCamera;

/// The shared preview render target plus the per-slot crop rectangles the cards
/// use to show each character. Rebuilt whenever the character list changes.
#[derive(Resource, Default)]
pub struct CharacterDiorama {
    /// Render target the preview camera draws into, or `None` when no slot is occupied.
    pub target: Option<Handle<Image>>,
    /// Occupied character slot -> its crop rectangle (pixels) within `target`.
    pub columns: HashMap<u8, Rect>,
    /// `(slot, char_id)` of the currently-rendered roster, so repeated identical
    /// `CharacterListReceivedEvent`s don't trigger a needless rebuild.
    signature: Option<Vec<(u8, u32)>>,
}

pub struct CharacterPreviewPlugin;

impl Plugin for CharacterPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterDiorama>();
        // `rebuild_diorama` spawns the preview entities (deferred) and writes their
        // `SpawnCharacterSpriteEvent` in one frame. Ordering it AFTER the engine's
        // `forward_character_sprite_events` means that event is only read the NEXT
        // frame — after the entity's components have flushed — so the lookup succeeds
        // and the sprite actually builds (otherwise the event is consumed against a
        // not-yet-existing entity and the preview never gets a sprite). Mirrors the
        // character-create screen's preview ordering.
        app.add_systems(
            Update,
            rebuild_diorama
                .after(forward_character_sprite_events)
                .run_if(in_state(GameState::CharacterSelection)),
        );
        app.add_systems(OnExit(GameState::CharacterSelection), cleanup_diorama);
    }
}

/// World-space spacing between adjacent characters, chosen so each character maps
/// to exactly one `COLUMN_PX`-wide column under the orthographic projection.
fn column_spacing() -> f32 {
    PREVIEW_VIEWPORT_HEIGHT * COLUMN_PX as f32 / ROW_PX as f32
}

/// Builds an empty RGBA render-target image with the usage flags a camera needs.
pub fn create_render_target(width: u32, height: u32) -> Image {
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    image
}

/// Crop rectangle in the shared target for the character in the `col`-th column.
fn column_rect(col: usize) -> Rect {
    let x = col as f32 * COLUMN_PX as f32;
    Rect::new(x, 0.0, x + COLUMN_PX as f32, ROW_PX as f32)
}

/// Rebuilds the diorama from the latest character list: despawns the previous
/// previews + camera, spawns one preview character per occupied slot in a row, and
/// points a single orthographic camera at the row, rendering into a fresh target.
fn rebuild_diorama(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut list_events: MessageReader<CharacterListReceivedEvent>,
    mut sprite_events: MessageWriter<SpawnCharacterSpriteEvent>,
    mut diorama: ResMut<CharacterDiorama>,
    previews: Query<Entity, With<PreviewCharacter>>,
    cameras: Query<Entity, With<PreviewCamera>>,
) {
    let Some(event) = list_events.read().last() else {
        return;
    };

    let occupied: Vec<(u8, &CharacterInfoWithJobName)> = event
        .characters
        .iter()
        .enumerate()
        .filter_map(|(slot, info)| info.as_ref().map(|info| (slot as u8, info)))
        .collect();

    // The engine emits `CharacterListReceivedEvent` more than once per visit (char-
    // server connect, then the screen's own list request). Rebuilding on each one
    // despawns the preview characters before their sprites finalize, so they'd never
    // render — skip when the roster is unchanged.
    let signature: Vec<(u8, u32)> = occupied
        .iter()
        .map(|(slot, info)| (*slot, info.base.char_id))
        .collect();
    if diorama.signature.as_deref() == Some(signature.as_slice()) {
        return;
    }
    diorama.signature = Some(signature);

    for entity in &previews {
        commands.entity(entity).despawn();
    }
    for entity in &cameras {
        commands.entity(entity).despawn();
    }
    diorama.columns.clear();
    diorama.target = None;

    if occupied.is_empty() {
        return;
    }

    let count = occupied.len() as u32;
    let target = images.add(create_render_target(count * COLUMN_PX, ROW_PX));
    let spacing = column_spacing();
    let row_center_x = (count as f32 - 1.0) * spacing / 2.0;

    for (col, (slot, info)) in occupied.iter().enumerate() {
        let position = Vec3::new(col as f32 * spacing, 0.0, 0.0);
        let (data, appearance, meta) = CharacterInfo::from(info.base.clone()).into_components();

        let entity = commands
            .spawn((
                data,
                appearance,
                meta,
                CharacterSprite::default(),
                CharacterDirection::default(),
                Transform::from_translation(position),
                Visibility::default(),
                PreviewCharacter,
                Name::new(format!("PreviewCharacter_slot{slot}")),
            ))
            .id();

        sprite_events.write(SpawnCharacterSpriteEvent {
            character_entity: entity,
            spawn_position: position,
        });

        diorama.columns.insert(*slot, column_rect(col));
    }

    let look_at = Vec3::new(row_center_x, LOOK_AT_Y, 0.0);
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
                viewport_height: PREVIEW_VIEWPORT_HEIGHT,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(look_at + CAMERA_OFFSET).looking_at(look_at, Vec3::NEG_Y),
        PreviewCamera,
        Name::new("CharacterPreviewCamera"),
    ));

    diorama.target = Some(target);
}

/// Tears the diorama down when leaving the character-selection screen so the
/// preview camera never coexists with the in-game `Camera3d`.
fn cleanup_diorama(
    mut commands: Commands,
    mut diorama: ResMut<CharacterDiorama>,
    previews: Query<Entity, With<PreviewCharacter>>,
    cameras: Query<Entity, With<PreviewCamera>>,
) {
    for entity in &previews {
        commands.entity(entity).despawn();
    }
    for entity in &cameras {
        commands.entity(entity).despawn();
    }
    diorama.columns.clear();
    diorama.target = None;
    diorama.signature = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_contiguous_and_sized() {
        let r0 = column_rect(0);
        let r1 = column_rect(1);
        assert_eq!(r0.min, Vec2::new(0.0, 0.0));
        assert_eq!(r0.max, Vec2::new(COLUMN_PX as f32, ROW_PX as f32));
        assert_eq!(r1.min.x, COLUMN_PX as f32);
        assert_eq!(r1.max.x, 2.0 * COLUMN_PX as f32);
    }

    #[test]
    fn spacing_maps_one_character_per_column() {
        // Horizontal world extent shown == count * spacing, so each column is one
        // character wide: extent = viewport_height * aspect, aspect = width/height.
        let count = 3u32;
        let aspect = (count * COLUMN_PX) as f32 / ROW_PX as f32;
        let world_extent = PREVIEW_VIEWPORT_HEIGHT * aspect;
        assert!((world_extent - count as f32 * column_spacing()).abs() < 1e-3);
    }
}
