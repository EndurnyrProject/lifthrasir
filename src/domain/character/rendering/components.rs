use crate::domain::character::CharacterData;
use crate::infrastructure::assets::loaders::{RoActAsset, RoPaletteAsset, RoSpriteAsset};
use bevy::prelude::*;

/// Simplified character rendering component for character selection screen
#[derive(Component, Debug)]
pub struct CharacterSelectionSprite {
    pub character_data: CharacterData,
    pub asset_state: CharacterAssetState,
    pub animation_timer: Timer,
    pub current_frame: usize,
    pub animation_type: CharacterAnimation,
}

impl CharacterSelectionSprite {
    pub fn new(character_data: CharacterData) -> Self {
        Self {
            character_data,
            asset_state: CharacterAssetState::NeedsLoading,
            animation_timer: Timer::from_seconds(0.15, TimerMode::Repeating), // 150ms per frame
            current_frame: 0,
            animation_type: CharacterAnimation::Idle,
        }
    }

    pub fn start_hover_animation(&mut self) {
        if self.animation_type == CharacterAnimation::Idle {
            self.animation_type = CharacterAnimation::Walking;
            self.current_frame = 0;
            self.animation_timer.reset();
        }
    }

    pub fn stop_hover_animation(&mut self) {
        if self.animation_type == CharacterAnimation::Walking {
            self.animation_type = CharacterAnimation::Idle;
            self.current_frame = 0;
            self.animation_timer.reset();
        }
    }
}

/// Tracks the state of asset loading for a character
#[derive(Debug, Clone)]
pub enum CharacterAssetState {
    NeedsLoading,
    Loading(Entity), // Store the request entity ID
    Ready(LoadedCharacterAssets),
}

/// Container for loaded character assets
#[derive(Debug, Clone)]
pub struct LoadedCharacterAssets {
    pub body_sprite: Handle<RoSpriteAsset>,
    pub body_act: Handle<RoActAsset>,
    pub body_palette: Option<Handle<RoPaletteAsset>>,
    pub head_sprite: Handle<RoSpriteAsset>,
    pub head_act: Handle<RoActAsset>,
    pub head_palette: Option<Handle<RoPaletteAsset>>,
}

/// Component to track pending asset loads for character sprites
#[derive(Component, Debug)]
pub struct PendingCharacterAssets {
    pub body_sprite: Handle<RoSpriteAsset>,
    pub body_act: Handle<RoActAsset>,
    pub head_sprite: Handle<RoSpriteAsset>,
    pub head_act: Handle<RoActAsset>,
    pub head_palette: Option<Handle<RoPaletteAsset>>,
}

/// Animation types for character selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterAnimation {
    Idle,    // Action index 0
    Walking, // Action index 1 (for hover effect)
}

impl CharacterAnimation {
    pub fn action_index(&self) -> usize {
        match self {
            CharacterAnimation::Idle => 0,
            CharacterAnimation::Walking => 1,
        }
    }
}

/// Component to link UI containers to character sprite entities
#[derive(Component)]
pub struct CharacterSpriteContainer {
    pub slot: u8,
    pub character_entity: Option<Entity>,
}

impl CharacterSpriteContainer {
    pub fn new(slot: u8) -> Self {
        Self {
            slot,
            character_entity: None,
        }
    }
}

/// Resource for tracking character selection rendering state
#[derive(Resource, Default)]
pub struct CharacterSelectionRenderState {
    pub loading_characters: Vec<(u8, Entity)>, // slot, character_entity
    pub rendered_characters: std::collections::HashMap<u8, Entity>,
}

impl CharacterSelectionRenderState {
    pub fn add_loading_character(&mut self, slot: u8, entity: Entity) {
        self.loading_characters.push((slot, entity));
    }

    pub fn mark_character_ready(&mut self, slot: u8, entity: Entity) {
        // Remove from loading list
        self.loading_characters.retain(|(s, _)| *s != slot);
        // Add to rendered list
        self.rendered_characters.insert(slot, entity);
    }

    pub fn is_character_rendered(&self, slot: u8) -> bool {
        self.rendered_characters.contains_key(&slot)
    }
}
