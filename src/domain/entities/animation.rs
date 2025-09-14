use crate::domain::entities::components::RoAnimationController;
use crate::infrastructure::assets::{
    RoActAsset, RoSpriteAsset, calculate_sprite_scale, convert_indexed_to_rgba, create_bevy_image,
};
use crate::utils::MAX_DISPLAYED_ACTIONS;
use bevy::prelude::*;

/// Animation system for RO sprites - ready for map entities
pub fn animate_sprites(
    mut commands: Commands,
    mut query: Query<(Entity, &mut RoAnimationController, &Transform)>,
    ro_sprites: Res<Assets<RoSpriteAsset>>,
    ro_actions: Res<Assets<RoActAsset>>,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time>,
) {
    for (entity, mut controller, _transform) in query.iter_mut() {
        let sprite_asset = ro_sprites.get(&controller.sprite_handle);
        let action_asset = ro_actions.get(&controller.action_handle);

        if let (Some(sprite_asset), Some(action_asset)) = (sprite_asset, action_asset) {
            let sprite = &sprite_asset.sprite;
            let action = &action_asset.action;

            // Update timer
            controller.timer += time.delta().as_millis() as f32;

            // Check if we need to advance to next frame
            if controller.timer >= controller.current_delay {
                controller.timer = 0.0;

                // Get current action sequence
                if let Some(action_seq) = action.actions.get(controller.action_index) {
                    // Advance animation index
                    controller.animation_index += 1;
                    if controller.animation_index >= action_seq.animations.len() {
                        controller.animation_index = 0;
                        // Move to next action
                        controller.action_index += 1;
                        if controller.action_index
                            >= action.actions.len().min(MAX_DISPLAYED_ACTIONS)
                        {
                            controller.action_index = 0; // Loop back to first action
                        }
                    }

                    // Update delay for next frame
                    controller.current_delay = action_seq.delay;
                }
            }

            // Get current animation and its first layer to determine sprite frame
            if let Some(action_seq) = action.actions.get(controller.action_index) {
                if let Some(animation) = action_seq.animations.get(controller.animation_index) {
                    if let Some(first_layer) = animation.layers.first() {
                        let sprite_index = first_layer.sprite_index as usize;

                        // Ensure sprite index is valid
                        if let Some(sprite_frame) = sprite.frames.get(sprite_index) {
                            // Convert sprite frame to Bevy Image
                            let rgba_data = if sprite_frame.is_rgba {
                                sprite_frame.data.clone()
                            } else if let Some(palette) = &sprite.palette {
                                convert_indexed_to_rgba(&sprite_frame.data, palette)
                            } else {
                                sprite_frame
                                    .data
                                    .iter()
                                    .flat_map(|&pixel| [pixel, pixel, pixel, 255])
                                    .collect()
                            };

                            let bevy_image = create_bevy_image(
                                sprite_frame.width as u32,
                                sprite_frame.height as u32,
                                rgba_data,
                            );

                            let image_handle = images.add(bevy_image);

                            // Calculate scale
                            let scale = calculate_sprite_scale(
                                sprite_frame.width as u32,
                                sprite_frame.height as u32,
                            );

                            // Apply layer positioning offset relative to current transform
                            let layer_offset = Vec3::new(
                                first_layer.pos[0] as f32,
                                first_layer.pos[1] as f32,
                                0.0,
                            );

                            // Use entity's current position plus layer offset
                            let new_transform =
                                Transform::from_translation(_transform.translation + layer_offset)
                                    .with_scale(Vec3::splat(scale));

                            // Update entity with new sprite and transform
                            commands
                                .entity(entity)
                                .insert((Sprite::from_image(image_handle), new_transform));

                            // Update frame index for tracking
                            controller.frame_index = sprite_index;
                        }
                    }
                }
            }
        }
    }
}
