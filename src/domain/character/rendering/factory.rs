use crate::domain::entities::components::RoAnimationController;
use crate::infrastructure::assets::loaders::{RoActAsset, RoPaletteAsset, RoSpriteAsset};
use bevy::prelude::*;
use bevy_lunex::prelude::*;

/// Marker component for sprite layers
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub enum CharacterSpriteLayer {
    Body,
    Head,
    HeadBottom,
    HeadMid,
    HeadTop,
}

/// Core sprite factory service - converts RO assets to Bevy sprites
pub struct CharacterSpriteFactory;

impl CharacterSpriteFactory {
    /// Creates character sprite entities and parents them to the given entity
    pub fn create_character_sprites(
        commands: &mut Commands,
        parent_entity: Entity,
        body_assets: Option<(
            &Handle<RoSpriteAsset>,
            &Handle<RoActAsset>,
            Option<&Handle<RoPaletteAsset>>,
        )>,
        head_assets: Option<(
            &Handle<RoSpriteAsset>,
            &Handle<RoActAsset>,
            Option<&Handle<RoPaletteAsset>>,
        )>,
        sprites: &Assets<RoSpriteAsset>,
        actions: &Assets<RoActAsset>,
        _palettes: &Assets<RoPaletteAsset>,
        _images: &mut Assets<Image>,
    ) -> Result<Vec<Entity>, String> {
        let mut sprite_entities = Vec::new();

        // Create body sprite if assets are available
        if let Some((spr_handle, act_handle, pal_handle)) = body_assets {
            let layout = Self::create_sprite_layout_from_act(
                CharacterSpriteLayer::Body,
                spr_handle,
                act_handle,
                sprites,
                actions,
            )?;

            let sprite_entity = Self::create_sprite_layer_with_controller(
                commands,
                CharacterSpriteLayer::Body,
                spr_handle.clone(),
                act_handle.clone(),
                pal_handle.cloned(),
                layout,
            );

            commands.entity(parent_entity).add_child(sprite_entity);
            sprite_entities.push(sprite_entity);
        }

        // Create head sprite if assets are available
        if let Some((spr_handle, act_handle, pal_handle)) = head_assets {
            let layout = Self::create_sprite_layout_from_act(
                CharacterSpriteLayer::Head,
                spr_handle,
                act_handle,
                sprites,
                actions,
            )?;

            let sprite_entity = Self::create_sprite_layer_with_controller(
                commands,
                CharacterSpriteLayer::Head,
                spr_handle.clone(),
                act_handle.clone(),
                pal_handle.cloned(),
                layout,
            );

            commands.entity(parent_entity).add_child(sprite_entity);
            sprite_entities.push(sprite_entity);
        }

        Ok(sprite_entities)
    }

    /// Creates UI layout from ACT positioning data
    fn create_sprite_layout_from_act(
        layer_type: CharacterSpriteLayer,
        sprite_handle: &Handle<RoSpriteAsset>,
        act_handle: &Handle<RoActAsset>,
        sprites: &Assets<RoSpriteAsset>,
        actions: &Assets<RoActAsset>,
    ) -> Result<UiLayout, String> {
        let ro_sprite = sprites
            .get(sprite_handle)
            .ok_or("Sprite asset not loaded")?;
        let ro_act = actions.get(act_handle).ok_or("ACT asset not loaded")?;

        // Get the first action (idle animation) and first frame
        let action_seq = ro_act
            .action
            .actions
            .first()
            .ok_or("No actions in ACT file")?;
        let animation = action_seq
            .animations
            .first()
            .ok_or("No animations in action sequence")?;
        let first_layer = animation.layers.first().ok_or("No layers in animation")?;

        // Get sprite frame for size information
        let sprite_index = if first_layer.sprite_index < 0 {
            0
        } else {
            first_layer.sprite_index as usize
        };

        let sprite_frame = if sprite_index < ro_sprite.sprite.frames.len() {
            &ro_sprite.sprite.frames[sprite_index]
        } else {
            ro_sprite
                .sprite
                .frames
                .first()
                .ok_or("No sprite frames available")?
        };

        // Convert RO coordinates to UI layout coordinates
        // RO uses pixel coordinates with Y-negative going up
        // UI layout uses relative coordinates with Y-positive going down
        let ro_x = first_layer.pos[0] as f32;
        let ro_y = first_layer.pos[1] as f32;

        // Apply C# client formula: position.y + sprite_height / 2f
        // Convert from RO coordinate system to UI relative positioning
        // RO: Y negative = up, Y positive = down
        // UI: Y positive = down, Y negative = up
        let base_x = 50.0; // Center horizontally
        let base_y = match layer_type {
            CharacterSpriteLayer::Body => {
                // Body at reference position with ACT offset
                let y_offset = ro_y / 3.0; // Scale down the ACT offset
                50.0 + y_offset
            }
            CharacterSpriteLayer::Head => {
                // Head positioned relative to ACT offset
                // Since head has more negative Y (-67) than body (-25), it should be higher
                let y_offset = ro_y / 3.0; // Use same scaling as body
                50.0 + y_offset
            }
            _ => 50.0, // Default to center for other layers
        };

        Ok(UiLayout::window()
            .pos(Rl((base_x, base_y)))
            .anchor(Anchor::Center)
            .size((Ab(64.0), Ab(64.0)))
            .pack())
    }

    /// Creates Transform from ACT positioning data
    fn create_transform_from_act(
        layer_type: CharacterSpriteLayer,
        sprite_handle: &Handle<RoSpriteAsset>,
        act_handle: &Handle<RoActAsset>,
        sprites: &Assets<RoSpriteAsset>,
        actions: &Assets<RoActAsset>,
    ) -> Result<Transform, String> {
        let ro_act = actions.get(act_handle).ok_or("ACT asset not loaded")?;

        // Get the first action (idle animation) and first frame
        let action_seq = ro_act
            .action
            .actions
            .first()
            .ok_or("No actions in ACT file")?;
        let animation = action_seq
            .animations
            .first()
            .ok_or("No animations in action sequence")?;
        let first_layer = animation.layers.first().ok_or("No layers in animation")?;

        // Convert ACT position to world space coordinates
        let ro_x = first_layer.pos[0] as f32;
        let ro_y = first_layer.pos[1] as f32;

        // Convert RO coordinates to Bevy world coordinates
        // RO sprites are positioned relative to character center
        let world_x = ro_x;
        let world_y = -ro_y; // Invert Y for Bevy coordinate system
        let world_z = match layer_type {
            CharacterSpriteLayer::Body => 0.0,
            CharacterSpriteLayer::Head => 0.1, // Slightly in front
            _ => 0.0,
        };

        // Transform positioning for proper sprite layering

        Ok(Transform::from_xyz(world_x, world_y, world_z))
    }

    /// Creates a sprite layer entity with RoAnimationController
    fn create_sprite_layer_with_controller(
        commands: &mut Commands,
        layer_type: CharacterSpriteLayer,
        sprite_handle: Handle<RoSpriteAsset>,
        act_handle: Handle<RoActAsset>,
        palette_handle: Option<Handle<RoPaletteAsset>>,
        layout: UiLayout,
    ) -> Entity {
        // Create animation controller with idle action (0)
        // Start paused for static display in character selection
        let mut controller = RoAnimationController::new(sprite_handle, act_handle)
            .with_action(0) // Idle action
            .looping(true)
            .paused(true); // Start paused, will unpause on hover for walking animation

        if let Some(palette) = palette_handle {
            controller = controller.with_palette(palette);
        }

        // Set proper Z-ordering for sprite layering (head in front of body)
        let transform = Transform::from_xyz(
            0.0,
            0.0,
            match layer_type {
                CharacterSpriteLayer::Body => 0.0,
                CharacterSpriteLayer::Head => 0.1, // Render in front of body
                CharacterSpriteLayer::HeadBottom => 0.05,
                CharacterSpriteLayer::HeadMid => 0.15,
                CharacterSpriteLayer::HeadTop => 0.2,
            },
        );

        // Spawn entity with controller - animate_sprites system will handle rendering
        commands
            .spawn((
                layout,
                controller,
                Sprite::default(), // Will be updated by animate_sprites system
                transform,
                GlobalTransform::default(),
                Visibility::default(),
                ViewVisibility::default(),
                InheritedVisibility::default(),
                layer_type,
                Name::new(format!("{:?}", layer_type)),
            ))
            .id()
    }
}
