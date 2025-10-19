use super::equipment::EquipmentSlot;
use crate::infrastructure::assets::loaders::{RoActAsset, RoSpriteAsset};
use bevy::prelude::*;
use std::collections::HashMap;

// Re-export Direction from coordinates module for convenience
pub use crate::utils::coordinates::Direction;

#[derive(Component, Debug)]
pub struct CharacterSprite {
    pub body_sprite: Entity,
    pub head_sprite: Entity,
    pub equipment_layers: HashMap<EquipmentSlot, Entity>,
    pub effect_layers: Vec<Entity>,
    pub current_action: u8,
    pub current_frame: u8,
    pub animation_timer: Timer,
}

#[derive(Component, Debug, Clone)]
pub struct RoSpriteLayer {
    pub sprite_handle: Handle<RoSpriteAsset>,
    pub action_handle: Handle<RoActAsset>,
    pub layer_type: SpriteLayerType,
    pub z_offset: f32,
}

#[derive(Debug, Clone)]
pub enum SpriteLayerType {
    Body,
    Head,
    Equipment(EquipmentSlot),
    Effect(EffectType),
    Shadow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectType {
    Blessing,
    Curse,
    Poison,
    Freeze,
    Stone,
    Stun,
    Sleep,
    Concentration,
    Endure,
    // Add more status effects as needed
}

#[derive(Component, Debug)]
pub struct CharacterDirection {
    pub facing: Direction,
}

impl Default for CharacterSprite {
    fn default() -> Self {
        Self {
            body_sprite: Entity::PLACEHOLDER,
            head_sprite: Entity::PLACEHOLDER,
            equipment_layers: HashMap::new(),
            effect_layers: Vec::new(),
            current_action: 0, // Idle action
            current_frame: 0,
            animation_timer: Timer::from_seconds(0.15, TimerMode::Repeating),
        }
    }
}

impl Default for CharacterDirection {
    fn default() -> Self {
        Self {
            facing: Direction::South,
        }
    }
}

impl CharacterSprite {
    pub fn has_effect_visual(&self, _effect_type: EffectType) -> bool {
        // For now, we'll implement this simply
        // In a full implementation, we'd check if any effect layer
        // corresponds to the given effect type
        false
    }

    /// Play an action with the given direction.
    ///
    /// This method calculates the correct ACT action index using the action mapping system
    /// and resets the animation state to start from the beginning of the new action.
    ///
    /// # Arguments
    ///
    /// * `action` - The action type to play (Idle, Walk, Attack, etc.)
    /// * `direction` - The direction to face (South, North, East, etc.)
    pub fn play_action(&mut self, action: ActionType, direction: Direction) {
        let action_index = super::action_mapping::calculate_action_index(action, direction);
        let old_action = self.current_action;
        self.current_action = action_index as u8;
        self.current_frame = 0;
        self.animation_timer.reset();

        debug!(
            "🎬 CharacterSprite.play_action: action={:?}, direction={:?}, old_action={}, new_action={}",
            action, direction, old_action, self.current_action
        );
    }

    /// Update the character's facing direction while maintaining the current action.
    ///
    /// This method smoothly transitions to the new direction without restarting
    /// the animation. The current frame and timer are preserved to avoid visual
    /// discontinuity when turning.
    ///
    /// # Arguments
    ///
    /// * `new_direction` - The new direction to face
    /// * `current_action_type` - The action type to maintain (obtained via get_current_action_type)
    pub fn update_direction(&mut self, new_direction: Direction, current_action_type: ActionType) {
        let new_action_index =
            super::action_mapping::calculate_action_index(current_action_type, new_direction);

        if new_action_index != self.current_action as usize {
            self.current_action = new_action_index as u8;
        }
    }

    /// Get the current action type from the current action index.
    ///
    /// This method reverse-maps from the internal action index back to the ActionType
    /// enum by checking which action range the index falls into.
    ///
    /// # Returns
    ///
    /// The ActionType corresponding to the current action index
    pub fn get_current_action_type(&self) -> ActionType {
        let index = self.current_action as usize;

        if index >= super::action_mapping::action_offsets::DEAD {
            ActionType::Dead
        } else if index >= super::action_mapping::action_offsets::HIT {
            ActionType::Hit
        } else if index >= super::action_mapping::action_offsets::ATTACK {
            ActionType::Attack
        } else if index >= super::action_mapping::action_offsets::PICKUP {
            ActionType::Cast
        } else if index >= super::action_mapping::action_offsets::SIT {
            ActionType::Sit
        } else if index >= super::action_mapping::action_offsets::WALK {
            ActionType::Walk
        } else {
            ActionType::Idle
        }
    }

    pub fn set_direction(&mut self, _direction: Direction) {
        // This would update the sprite direction
        // For now, just store it - actual implementation would
        // update the sprite rendering based on direction
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    Idle = 0,
    Walk = 1,
    Sit = 2,
    Attack = 3,
    Hit = 4,
    Dead = 5,
    Cast = 6,
    Special = 7,
}

impl From<EffectType> for SpriteLayerType {
    fn from(effect_type: EffectType) -> Self {
        SpriteLayerType::Effect(effect_type)
    }
}

impl SpriteLayerType {
    pub fn from_name(name: &str) -> Self {
        match name {
            "Body" => SpriteLayerType::Body,
            "Head" => SpriteLayerType::Head,
            "Equipment/HeadBottom" => SpriteLayerType::Equipment(EquipmentSlot::HeadBottom),
            "Equipment/HeadMid" => SpriteLayerType::Equipment(EquipmentSlot::HeadMid),
            "Equipment/HeadTop" => SpriteLayerType::Equipment(EquipmentSlot::HeadTop),
            _ => SpriteLayerType::Body, // Default fallback
        }
    }
}

impl Default for RoSpriteLayer {
    fn default() -> Self {
        Self {
            sprite_handle: Handle::default(),
            action_handle: Handle::default(),
            layer_type: SpriteLayerType::Body,
            z_offset: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_play_action_sets_correct_index() {
        let mut sprite = CharacterSprite::default();

        sprite.play_action(ActionType::Walk, Direction::North);
        assert_eq!(sprite.current_action, 12);
        assert_eq!(sprite.current_frame, 0);

        sprite.play_action(ActionType::Attack, Direction::East);
        assert_eq!(sprite.current_action, 38);
        assert_eq!(sprite.current_frame, 0);
    }

    #[test]
    fn test_update_direction_preserves_action_type() {
        let mut sprite = CharacterSprite::default();

        sprite.play_action(ActionType::Walk, Direction::South);
        let initial_frame = sprite.current_frame;
        assert_eq!(sprite.current_action, 8);

        sprite.update_direction(Direction::North, ActionType::Walk);
        assert_eq!(sprite.current_action, 12);
        assert_eq!(sprite.current_frame, initial_frame);
    }

    #[test]
    fn test_update_direction_no_change_optimization() {
        let mut sprite = CharacterSprite::default();

        sprite.play_action(ActionType::Idle, Direction::South);
        let action_before = sprite.current_action;

        sprite.update_direction(Direction::South, ActionType::Idle);
        assert_eq!(sprite.current_action, action_before);
    }

    #[test]
    fn test_get_current_action_type_idle() {
        let mut sprite = CharacterSprite::default();

        for direction in [
            Direction::South,
            Direction::SouthWest,
            Direction::West,
            Direction::NorthWest,
            Direction::North,
            Direction::NorthEast,
            Direction::East,
            Direction::SouthEast,
        ] {
            sprite.play_action(ActionType::Idle, direction);
            assert_eq!(sprite.get_current_action_type(), ActionType::Idle);
        }
    }

    #[test]
    fn test_get_current_action_type_walk() {
        let mut sprite = CharacterSprite::default();

        for direction in [
            Direction::South,
            Direction::SouthWest,
            Direction::West,
            Direction::NorthWest,
            Direction::North,
            Direction::NorthEast,
            Direction::East,
            Direction::SouthEast,
        ] {
            sprite.play_action(ActionType::Walk, direction);
            assert_eq!(sprite.get_current_action_type(), ActionType::Walk);
        }
    }

    #[test]
    fn test_get_current_action_type_all_actions() {
        let mut sprite = CharacterSprite::default();

        let action_types = [
            (ActionType::Idle, Direction::South),
            (ActionType::Walk, Direction::North),
            (ActionType::Sit, Direction::East),
            (ActionType::Attack, Direction::West),
            (ActionType::Hit, Direction::NorthEast),
            (ActionType::Dead, Direction::SouthWest),
            (ActionType::Cast, Direction::NorthWest),
        ];

        for (action_type, direction) in action_types.iter() {
            sprite.play_action(*action_type, *direction);
            assert_eq!(sprite.get_current_action_type(), *action_type);
        }
    }

    #[test]
    fn test_round_trip_action_type_conversion() {
        let mut sprite = CharacterSprite::default();

        let test_cases = [
            (ActionType::Idle, Direction::South),
            (ActionType::Walk, Direction::North),
            (ActionType::Sit, Direction::East),
            (ActionType::Attack, Direction::West),
            (ActionType::Hit, Direction::NorthWest),
            (ActionType::Dead, Direction::SouthEast),
            (ActionType::Cast, Direction::NorthEast),
        ];

        for (action, direction) in test_cases.iter() {
            sprite.play_action(*action, *direction);
            let retrieved_action = sprite.get_current_action_type();
            assert_eq!(retrieved_action, *action);
        }
    }
}
