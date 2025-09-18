use crate::domain::character::rendering::{
    CharacterAnimation, CharacterAssetState, CharacterSelectionSprite, CharacterSpriteFactory,
    LoadedCharacterAssets, PendingCharacterAssets,
};
use crate::domain::entities::components::RoAnimationController;
use bevy::prelude::*;
use crate::infrastructure::assets::loaders::{RoActAsset, RoPaletteAsset, RoSpriteAsset};

/// System to request asset loading for characters that need it
pub fn request_character_assets_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut query: Query<(Entity, &mut CharacterSelectionSprite)>,
) {
    for (entity, mut sprite) in query.iter_mut() {
        if let CharacterAssetState::NeedsLoading = sprite.asset_state {
            info!(
                "Loading assets for character: {}",
                sprite.character_data.name
            );

            // Generate sprite paths
            let paths = crate::domain::character::rendering::paths::generate_character_sprite_paths(&sprite.character_data);

            // Load assets directly using AssetServer with "ro://" prefix
            let body_sprite: Handle<RoSpriteAsset> = asset_server.load(format!("ro://{}", paths.body_sprite));
            let body_act: Handle<RoActAsset> = asset_server.load(format!("ro://{}", paths.body_act));
            let head_sprite: Handle<RoSpriteAsset> = asset_server.load(format!("ro://{}", paths.head_sprite));
            let head_act: Handle<RoActAsset> = asset_server.load(format!("ro://{}", paths.head_act));
            let head_palette = paths.head_palette.as_ref().map(|path| {
                asset_server.load::<RoPaletteAsset>(format!("ro://{}", path))
            });

            // Add pending component to track loading
            commands.entity(entity).insert(PendingCharacterAssets {
                body_sprite,
                body_act,
                head_sprite,
                head_act,
                head_palette,
            });

            // Update state to loading
            sprite.asset_state = CharacterAssetState::Loading(entity);
        }
    }
}

/// System to finalize character sprites once assets are loaded
pub fn finalize_character_sprites_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut query: Query<(Entity, &mut CharacterSelectionSprite, &PendingCharacterAssets)>,
    sprites: Res<Assets<RoSpriteAsset>>,
    actions: Res<Assets<RoActAsset>>,
    palettes: Res<Assets<RoPaletteAsset>>,
    mut images: ResMut<Assets<Image>>,
) {
    for (entity, mut sprite, pending) in query.iter_mut() {
        // Check if all required assets are loaded
        let body_sprite_loaded = sprites.get(&pending.body_sprite).is_some();
        let body_act_loaded = actions.get(&pending.body_act).is_some();
        let head_sprite_loaded = sprites.get(&pending.head_sprite).is_some();
        let head_act_loaded = actions.get(&pending.head_act).is_some();
        let palette_loaded = pending.head_palette.as_ref()
            .map(|h| palettes.get(h).is_some())
            .unwrap_or(true); // No palette = considered loaded

        if body_sprite_loaded && body_act_loaded && head_sprite_loaded && head_act_loaded && palette_loaded {
            info!(
                "All assets loaded for character: {}, creating sprite entities",
                sprite.character_data.name
            );

            // Create the loaded assets container
            let loaded_assets = LoadedCharacterAssets {
                body_sprite: pending.body_sprite.clone(),
                body_act: pending.body_act.clone(),
                body_palette: pending.head_palette.clone(), // Use palette for body too
                head_sprite: pending.head_sprite.clone(),
                head_act: pending.head_act.clone(),
                head_palette: pending.head_palette.clone(),
            };

            // Create sprite entities
            let body_assets = Some((&pending.body_sprite, &pending.body_act, pending.head_palette.as_ref()));
            let head_assets = Some((&pending.head_sprite, &pending.head_act, pending.head_palette.as_ref()));

            match CharacterSpriteFactory::create_character_sprites(
                &mut commands,
                entity,
                body_assets,
                head_assets,
                &sprites,
                &actions,
                &palettes,
                &mut images,
            ) {
                Ok(sprite_entities) => {
                    info!(
                        "Created {} sprite entities for character: {}",
                        sprite_entities.len(),
                        sprite.character_data.name
                    );

                    // Update asset state to ready
                    sprite.asset_state = CharacterAssetState::Ready(loaded_assets);

                    // Remove pending component
                    commands.entity(entity).remove::<PendingCharacterAssets>();
                }
                Err(e) => {
                    warn!("Failed to create sprite entities: {}", e);
                    // Keep pending component and try again next frame
                }
            }
        }
    }
}

/// System to sync character selection animation state with RoAnimationController
/// The actual frame advancement is handled by animate_sprites system
pub fn animate_character_selection_sprites_system(
    mut query: Query<(&CharacterSelectionSprite, &Children)>,
    mut controller_query: Query<&mut RoAnimationController>,
) {
    for (character_sprite, children) in query.iter_mut() {
        // Only update if assets are ready
        if let CharacterAssetState::Ready(_assets) = &character_sprite.asset_state {
            // Get the action index for current animation type
            let action_index = character_sprite.animation_type.action_index();

            // Update all child sprite controllers with the new action index
            for child in children.iter() {
                if let Ok(mut controller) = controller_query.get_mut(child) {
                    // Only update if action changed
                    if controller.action_index != action_index {
                        controller.action_index = action_index;
                        controller.animation_index = 0;
                        controller.frame_index = 0;
                        controller.timer = 0.0;
                        // Unpause to enable animation (for hover effect)
                        controller.paused = false;
                    }
                }
            }
        }
    }
}

/// System to handle hover effects (switch to walking animation)
pub fn handle_character_hover_system(
    selection: Res<crate::presentation::ui::screens::character_selection::list::CharacterSelectionResource>,
    mut query: Query<(
        &mut CharacterSelectionSprite,
        &crate::domain::character::CharacterCard,
    )>,
) {
    for (mut sprite, card) in query.iter_mut() {
        if let Some(character) = &card.character {
            let is_hovered = selection.hovering_slot == Some(card.slot);

            // Update animation based on hover state
            if is_hovered && sprite.animation_type == CharacterAnimation::Idle {
                sprite.start_hover_animation();
            } else if !is_hovered && sprite.animation_type == CharacterAnimation::Walking {
                sprite.stop_hover_animation();
            }
        }
    }
}
