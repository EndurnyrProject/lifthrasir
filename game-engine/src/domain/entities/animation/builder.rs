use crate::infrastructure::assets::{
    convert_sprite_frame_to_rgba, create_bevy_image, RoActAsset, RoAnimationAsset, RoPaletteAsset,
    RoSpriteAsset,
};
use bevy::prelude::*;

/// Builder for creating RoAnimationAsset from legacy SPR/ACT assets
pub struct AnimationAssetBuilder;

impl AnimationAssetBuilder {
    /// Convert legacy SPR+ACT into RoAnimationAsset
    /// This can be called during asset loading or on-demand
    pub fn build_from_legacy(
        sprite: &RoSpriteAsset,
        action: &RoActAsset,
        action_index: usize,
        images: &mut Assets<Image>,
        custom_palette: Option<&RoPaletteAsset>,
    ) -> Option<RoAnimationAsset> {
        let action_seq = action.action.actions.get(action_index)?;

        let mut frame_handles = Vec::new();
        let mut frame_offsets = Vec::new();

        for animation in &action_seq.animations {
            let Some(first_layer) = animation.layers.first() else {
                continue;
            };

            let sprite_index = first_layer.sprite_index.max(0) as usize;
            let Some(sprite_frame) = sprite.sprite.frames.get(sprite_index) else {
                continue;
            };

            let rgba_data = convert_sprite_frame_to_rgba(
                sprite_frame,
                sprite.sprite.palette.as_ref(),
                custom_palette,
            );

            let image = create_bevy_image(
                sprite_frame.width as u32,
                sprite_frame.height as u32,
                rgba_data,
            );

            let handle = images.add(image);
            frame_handles.push(handle);

            frame_offsets.push((first_layer.pos[0] as f32, first_layer.pos[1] as f32));
        }

        if frame_handles.is_empty() {
            return None;
        }

        let frame_duration = std::time::Duration::from_millis(action_seq.delay.max(100.0) as u64);

        Some(RoAnimationAsset::new(
            frame_handles,
            frame_duration,
            true,
            frame_offsets,
        ))
    }
}
