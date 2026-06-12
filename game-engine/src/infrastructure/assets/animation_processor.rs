use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use moonshine_tag::Tag;

use crate::infrastructure::ro_formats::act::{Layer, RoAction};
use crate::infrastructure::ro_formats::sprite::{Palette, RoSprite, SpriteFrame};

use super::converters::{apply_magenta_transparency, convert_sprite_frame_to_rgba};
use super::ro_animation_asset::{ActionData, FrameData, FramePart, RoAnimationAsset};

pub struct RoAnimationProcessor;

impl RoAnimationProcessor {
    /// Process a single SPR+ACT pair into a RoAnimationAsset.
    /// Each layer (body, head, weapon) is processed separately.
    pub fn process(
        sprite: &RoSprite,
        action: &RoAction,
        layer_tag: Tag,
        images: &mut Assets<Image>,
    ) -> RoAnimationAsset {
        let textures = Self::create_textures(sprite, images);
        let actions = Self::create_actions(action, sprite);

        RoAnimationAsset {
            textures,
            actions,
            layer: layer_tag,
        }
    }

    /// Convert all sprite frames to GPU textures once during loading.
    fn create_textures(sprite: &RoSprite, images: &mut Assets<Image>) -> Vec<Handle<Image>> {
        let handles: Vec<_> = sprite
            .frames
            .iter()
            .map(|frame| {
                let image = Self::frame_to_image(frame, sprite.palette.as_ref());
                images.add(image)
            })
            .collect();
        if let Some(first) = handles.first() {
            bevy::log::info!(
                "create_textures: Created {} textures, first handle: {:?}",
                handles.len(),
                first
            );
        }
        handles
    }

    /// Convert a sprite frame to a Bevy Image.
    fn frame_to_image(frame: &SpriteFrame, palette: Option<&Palette>) -> Image {
        let mut rgba_data = convert_sprite_frame_to_rgba(frame, palette, None);
        apply_magenta_transparency(&mut rgba_data);

        Image::new(
            Extent3d {
                width: frame.width as u32,
                height: frame.height as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba_data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        )
    }

    /// Create ActionData for each action in the ACT file.
    /// Each ActionSequence in the ACT file represents ONE direction of ONE action.
    /// The action index formula is: base_action * 8 + direction
    fn create_actions(action: &RoAction, sprite: &RoSprite) -> Vec<ActionData> {
        action
            .actions
            .iter()
            .map(|action_seq| {
                let frames = Self::create_frames(action_seq, sprite);

                ActionData {
                    frames,
                    delay_ms: action_seq.delay,
                }
            })
            .collect()
    }

    /// Create FrameData for each animation frame in an action.
    fn create_frames(
        action_seq: &crate::infrastructure::ro_formats::act::ActionSequence,
        sprite: &RoSprite,
    ) -> Vec<FrameData> {
        action_seq
            .animations
            .iter()
            .map(|animation| {
                let parts = Self::create_frame_parts(&animation.layers, sprite);
                let (size, offset) = Self::calculate_bounds(&animation.layers, sprite);
                let attach_point = Self::extract_attach_point(animation);

                FrameData {
                    parts,
                    size,
                    offset,
                    attach_point,
                    sound_id: if animation.sound_id >= 0 {
                        Some(animation.sound_id)
                    } else {
                        None
                    },
                    is_attack_frame: false,
                }
            })
            .collect()
    }

    /// Create FramePart for each layer in a frame.
    fn create_frame_parts(layers: &[Layer], sprite: &RoSprite) -> Vec<FramePart> {
        layers
            .iter()
            .filter(|layer| layer.sprite_index >= 0)
            .filter(|layer| (layer.sprite_index as usize) < sprite.frames.len())
            .map(|layer| {
                let transform = Self::build_transform(layer, sprite);
                // Negate Y to convert from ACT coords (+Y up) to Bevy coords (-Y up)
                let position = Vec2::new(layer.pos[0] as f32, -layer.pos[1] as f32);
                let scale = Vec2::new(layer.scale[0], layer.scale[1]);
                let frame = &sprite.frames[layer.sprite_index as usize];
                let texture_size = Vec2::new(frame.width as f32, frame.height as f32);

                FramePart {
                    texture_index: layer.sprite_index as usize,
                    transform,
                    position,
                    scale,
                    texture_size,
                    color: Color::srgba(
                        layer.color[0],
                        layer.color[1],
                        layer.color[2],
                        layer.color[3],
                    ),
                    mirror: layer.is_mirror,
                }
            })
            .collect()
    }

    /// Build the affine transform matrix for a layer.
    fn build_transform(layer: &Layer, sprite: &RoSprite) -> Mat4 {
        let frame = sprite
            .frames
            .get(layer.sprite_index as usize)
            .expect("valid sprite index");

        let pos_x = layer.pos[0] as f32;
        let pos_y = layer.pos[1] as f32;
        let scale_x = layer.scale[0];
        let scale_y = layer.scale[1];
        let angle_deg = layer.angle as f32;

        let half_w = frame.width as f32 / 2.0;
        let half_h = frame.height as f32 / 2.0;

        let translation = Mat4::from_translation(Vec3::new(pos_x, -pos_y, 0.0));
        let rotation = Mat4::from_rotation_z(-angle_deg.to_radians());
        let scale = Mat4::from_scale(Vec3::new(
            scale_x * if layer.is_mirror { -1.0 } else { 1.0 },
            scale_y,
            1.0,
        ));
        let center_offset = Mat4::from_translation(Vec3::new(-half_w, -half_h, 0.0));

        translation * rotation * scale * center_offset
    }

    /// Calculate bounding box size and offset for a frame.
    fn calculate_bounds(layers: &[Layer], sprite: &RoSprite) -> (Vec2, Vec2) {
        if layers.is_empty() {
            return (Vec2::ZERO, Vec2::ZERO);
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for layer in layers {
            if layer.sprite_index < 0 {
                continue;
            }

            let Some(frame) = sprite.frames.get(layer.sprite_index as usize) else {
                continue;
            };

            let w = frame.width as f32 * layer.scale[0];
            let h = frame.height as f32 * layer.scale[1];
            let x = layer.pos[0] as f32;
            let y = layer.pos[1] as f32;

            min_x = min_x.min(x - w / 2.0);
            min_y = min_y.min(y - h / 2.0);
            max_x = max_x.max(x + w / 2.0);
            max_y = max_y.max(y + h / 2.0);
        }

        let size = Vec2::new(max_x - min_x, max_y - min_y);
        let offset = Vec2::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);

        (size, offset)
    }

    /// Extract attach point from animation frame (for body/head connection).
    /// Y is negated to convert from RO coordinates (+Y down) to Bevy coordinates (-Y up).
    fn extract_attach_point(
        animation: &crate::infrastructure::ro_formats::act::Animation,
    ) -> Option<Vec2> {
        animation
            .positions
            .first()
            .map(|pos| Vec2::new(pos.x as f32, -pos.y as f32))
    }
}

/// Calculate the head-to-body attachment offset.
/// Used at render time when compositing body and head sprites.
pub fn calculate_attach_offset(body_attach: Option<Vec2>, head_attach: Option<Vec2>) -> Vec2 {
    match (body_attach, head_attach) {
        (Some(body), Some(head)) => body - head,
        _ => Vec2::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_attach_offset() {
        let body = Some(Vec2::new(10.0, 20.0));
        let head = Some(Vec2::new(5.0, 15.0));
        let offset = calculate_attach_offset(body, head);
        assert_eq!(offset, Vec2::new(5.0, 5.0));
    }

    #[test]
    fn test_calculate_attach_offset_none() {
        assert_eq!(calculate_attach_offset(None, None), Vec2::ZERO);
        assert_eq!(
            calculate_attach_offset(Some(Vec2::new(10.0, 20.0)), None),
            Vec2::ZERO
        );
    }
}
