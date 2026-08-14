use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use moonshine_tag::Tag;

use crate::domain::settings::resources::Upscaling;
use crate::infrastructure::ro_formats::act::{Layer, RoAction};
use crate::infrastructure::ro_formats::sprite::{Palette, RoSprite, SpriteFrame};

use super::converters::{apply_magenta_transparency, convert_sprite_frame_to_rgba};
use super::loaders::RoPaletteAsset;
use super::ro_animation_asset::{ActionData, FrameData, FramePart, RoAnimationAsset};
use super::upscale;

pub struct RoAnimationProcessor;

/// Output of [`RoAnimationProcessor::process_cpu`]: fully converted frame
/// images plus animation metadata, produced off the main thread. Call
/// [`ProcessedAnimation::finalize`] on the main thread to register the images
/// and obtain the [`RoAnimationAsset`].
pub struct ProcessedAnimation {
    pub images: Vec<Image>,
    pub actions: Vec<ActionData>,
    pub layer: Tag,
    pub sounds: Vec<String>,
}

impl ProcessedAnimation {
    /// Register the pre-built images and assemble the final asset.
    /// Cheap: no pixel work happens here.
    pub fn finalize(self, images: &mut Assets<Image>) -> RoAnimationAsset {
        RoAnimationAsset {
            textures: self
                .images
                .into_iter()
                .map(|image| images.add(image))
                .collect(),
            actions: self.actions,
            layer: self.layer,
            sounds: self.sounds,
        }
    }
}

impl RoAnimationProcessor {
    /// Process a single SPR+ACT pair into a RoAnimationAsset.
    /// Each layer (body, head, weapon) is processed separately.
    pub fn process(
        sprite: &RoSprite,
        action: &RoAction,
        custom_palette: Option<&RoPaletteAsset>,
        layer_tag: Tag,
        images: &mut Assets<Image>,
        upscaling: Upscaling,
    ) -> RoAnimationAsset {
        Self::process_cpu(sprite, action, custom_palette, layer_tag, upscaling).finalize(images)
    }

    /// CPU-only stage: palette lookup, RGBA conversion, and xBRZ upscaling.
    /// Touches no `Assets` storage, so it is safe to run on `AsyncComputeTaskPool`.
    pub fn process_cpu(
        sprite: &RoSprite,
        action: &RoAction,
        custom_palette: Option<&RoPaletteAsset>,
        layer_tag: Tag,
        upscaling: Upscaling,
    ) -> ProcessedAnimation {
        let images = sprite
            .frames
            .iter()
            .map(|frame| {
                Self::frame_to_image(frame, sprite.palette.as_ref(), custom_palette, upscaling)
            })
            .collect();

        ProcessedAnimation {
            images,
            actions: Self::create_actions(action, sprite),
            layer: layer_tag,
            sounds: action.sounds.clone(),
        }
    }

    /// Convert a sprite frame to a Bevy Image.
    fn frame_to_image(
        frame: &SpriteFrame,
        palette: Option<&Palette>,
        custom_palette: Option<&RoPaletteAsset>,
        upscaling: Upscaling,
    ) -> Image {
        let mut rgba_data = convert_sprite_frame_to_rgba(frame, palette, custom_palette);
        apply_magenta_transparency(&mut rgba_data);

        let (rgba_data, width, height) = upscale::scale(
            &rgba_data,
            frame.width as u32,
            frame.height as u32,
            upscaling,
        );

        Image::new(
            Extent3d {
                width,
                height,
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

    fn rgba_frame(width: u16, height: u16) -> SpriteFrame {
        SpriteFrame {
            width,
            height,
            data: vec![0x10; width as usize * height as usize * 4],
            is_rgba: true,
        }
    }

    #[test]
    fn frame_to_image_keeps_extent_when_off() {
        let frame = rgba_frame(2, 2);
        let image = RoAnimationProcessor::frame_to_image(&frame, None, None, Upscaling::Off);
        assert_eq!(image.texture_descriptor.size.width, 2);
        assert_eq!(image.texture_descriptor.size.height, 2);
    }

    #[test]
    fn frame_to_image_scales_pixels_but_not_logical_size() {
        let frame = rgba_frame(2, 2);
        let image = RoAnimationProcessor::frame_to_image(&frame, None, None, Upscaling::X2);
        assert_eq!(image.texture_descriptor.size.width, 4);
        assert_eq!(image.texture_descriptor.size.height, 4);
        assert_eq!((frame.width, frame.height), (2, 2));
    }

    #[test]
    fn frame_to_image_uses_custom_palette() {
        let frame = SpriteFrame {
            width: 1,
            height: 1,
            data: vec![1],
            is_rgba: false,
        };
        let embedded_palette = Palette {
            colors: vec![[0, 0, 0, 0], [1, 2, 3, 255]],
        };
        let custom_palette = super::super::loaders::RoPaletteAsset {
            colors: vec![[0, 0, 0, 0], [10, 20, 30, 255]],
        };

        let image = RoAnimationProcessor::frame_to_image(
            &frame,
            Some(&embedded_palette),
            Some(&custom_palette),
            Upscaling::Off,
        );

        assert_eq!(image.data.unwrap(), vec![10, 20, 30, 255]);
    }
}
