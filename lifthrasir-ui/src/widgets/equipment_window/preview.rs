//! Live, rotatable character preview for the equipment window.
//!
//! Renders the local player's actual sprite + equipped headgear into the window's
//! preview frame via an off-screen orthographic `Camera3d`, isolated from the world
//! camera by a dedicated [`RenderLayers`]. The preview character is a normal sprite
//! entity spawned on the preview layer; it animates through the exact same engine
//! pipeline as in-world characters (body, head, headgear, direction), so it always
//! mirrors the player.
//!
//! Isolation: the world is on the default render layer 0 (no engine entity sets
//! `RenderLayers`), so putting the preview camera and the preview billboards on layer
//! [`PREVIEW_LAYER`] keeps each camera blind to the other's content. The six engine
//! systems that resolve "the" `Camera3d` via `single()` exclude
//! [`EquipmentPreviewCamera`], so in-world head / headgear / direction / picking keep
//! working while this second camera exists; the preview character's own
//! head/headgear math reuses the world camera (whose rotation matches this camera's
//! fixed RO-isometric tilt), so it lines up too.

use std::collections::HashMap;

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{
    ClearColorConfig, OrthographicProjection, Projection, RenderTarget, ScalingMode,
};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
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

use super::{EquipmentPreviewFrame, EquipmentWindowRoot};

/// Render-target dimensions; 2x the preview frame's 90x120 box so the character
/// supersamples down crisply.
const PREVIEW_W: u32 = 180;
const PREVIEW_H: u32 = 240;
/// World-space vertical extent the orthographic camera frames; tuned in
/// `character_preview` so a standing character fills the frame.
const VIEWPORT_HEIGHT: f32 = 42.0;
/// Vertical aim offset so the camera frames the body, not the feet at the origin.
const LOOK_AT_Y: f32 = -8.0;
/// Camera offset mirroring the in-world RO isometric tilt
/// (`CameraFollowSettings::default().offset`), so this camera's rotation matches the
/// world follow camera's — that match is what lets the engine drive the preview
/// character's head/headgear from the world camera.
const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, -150.0, -150.0);
/// Where the preview rig lives, far from the play area: until a freshly spawned
/// billboard is tagged onto the preview layer it is briefly on layer 0, and placing
/// it here keeps that one frame off the world camera entirely.
const PREVIEW_ORIGIN: Vec3 = Vec3::new(0.0, 100_000.0, 0.0);
/// Dedicated render layer isolating the preview camera + billboards from the world.
const PREVIEW_LAYER: usize = 1;

/// Marker on the spawned preview character root.
#[derive(Component)]
pub struct PreviewCharacter;

/// Holds the preview render target so it is created once per in-game session.
#[derive(Resource, Default)]
pub struct PreviewState {
    target: Option<Handle<Image>>,
}

/// The local player's currently equipped headgear (`slot -> view id`), accumulated
/// from the equipment change stream so the preview can mirror it on spawn.
#[derive(Resource, Default)]
pub struct LocalHeadgear(HashMap<EquipmentSlot, u16>);

/// Track the local player's headgear from the equipment change stream. Only the
/// local player emits these (self-targeted), and only for headgear slots.
pub fn cache_local_headgear(
    mut changes: MessageReader<EquipmentChangeEvent>,
    local: Query<Entity, With<LocalPlayer>>,
    mut cache: ResMut<LocalHeadgear>,
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

/// Lazily build the preview the first time the window is shown: create the render
/// target + camera (once per session), bind it to the frame, and spawn the preview
/// character mirroring the local player. The character carries an [`EquipmentSet`]
/// built from the cached headgear so the engine's remote-headgear path renders it as
/// soon as the body/head children appear.
#[allow(clippy::too_many_arguments)]
pub fn spawn_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut state: ResMut<PreviewState>,
    mut spawn_events: MessageWriter<SpawnCharacterSpriteEvent>,
    window: Query<&Visibility, With<EquipmentWindowRoot>>,
    frame: Query<Entity, With<EquipmentPreviewFrame>>,
    existing: Query<(), With<PreviewCharacter>>,
    local: Query<(&CharacterData, &CharacterAppearance), With<LocalPlayer>>,
    cache: Res<LocalHeadgear>,
) {
    if !existing.is_empty() {
        return;
    }
    let Ok(visibility) = window.single() else {
        return;
    };
    if *visibility != Visibility::Visible {
        return;
    }
    let Ok((data, appearance)) = local.single() else {
        return;
    };

    if state.target.is_none() {
        let target = images.add(create_render_target(PREVIEW_W, PREVIEW_H));
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
            Name::new("EquipmentPreviewCamera"),
        ));

        if let Ok(frame) = frame.single() {
            commands.spawn((
                ImageNode::new(target.clone()),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(frame),
            ));
        }

        state.target = Some(target);
    }

    let entity = commands
        .spawn((
            data.clone(),
            appearance.clone(),
            equipment_set_from(&cache.0),
            CharacterSprite::default(),
            CharacterDirection::default(),
            Transform::from_translation(PREVIEW_ORIGIN),
            Visibility::default(),
            PreviewCharacter,
            Name::new("EquipmentPreviewCharacter"),
        ))
        .id();

    spawn_events.write(SpawnCharacterSpriteEvent {
        character_entity: entity,
        spawn_position: PREVIEW_ORIGIN,
    });
}

/// Forward the local player's live headgear changes onto the preview character so
/// equipping / unequipping updates the preview in place (no respawn).
pub fn forward_preview_headgear(
    mut messages: ParamSet<(
        MessageReader<EquipmentChangeEvent>,
        MessageWriter<EquipmentChangeEvent>,
    )>,
    local: Query<Entity, With<LocalPlayer>>,
    preview: Query<Entity, With<PreviewCharacter>>,
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
    preview: Query<&Children, With<PreviewCharacter>>,
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

/// Despawn the preview character + camera when leaving the game so no second camera
/// or sprite leaks into the next session. The frame's `ImageNode` rides the HUD root,
/// which is despawned with the rest of the in-game UI.
pub fn cleanup_preview(
    mut commands: Commands,
    mut state: ResMut<PreviewState>,
    mut cache: ResMut<LocalHeadgear>,
    characters: Query<Entity, With<PreviewCharacter>>,
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

pub fn on_rotate_left(
    _: On<Activate>,
    mut preview: Query<&mut CharacterDirection, With<PreviewCharacter>>,
) {
    if let Ok(mut direction) = preview.single_mut() {
        direction.facing = step_facing(direction.facing, -1);
    }
}

pub fn on_rotate_right(
    _: On<Activate>,
    mut preview: Query<&mut CharacterDirection, With<PreviewCharacter>>,
) {
    if let Ok(mut direction) = preview.single_mut() {
        direction.facing = step_facing(direction.facing, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
