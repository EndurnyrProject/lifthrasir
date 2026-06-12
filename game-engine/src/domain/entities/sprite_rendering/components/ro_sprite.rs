use std::marker::PhantomData;

use bevy::prelude::*;

use crate::domain::entities::character::components::visual::{ActionType, Direction};
use crate::domain::entities::sprite_rendering::layout::ActionLayout;
use crate::infrastructure::assets::ro_animation_asset::{FrameData, RoAnimationAsset};

#[derive(Component, Clone, Debug)]
pub struct RoSpriteGeneric<T: ActionLayout> {
    pub animation: Handle<RoAnimationAsset>,
    pub action_type: ActionType,
    pub direction: Direction,
    pub start_time: u32,
    pub speed_factor: f32,
    /// When set, the animation is stretched so all frames play exactly within
    /// this duration (used for attack animations driven by ASPD).
    pub fixed_duration_ms: Option<u32>,
    _marker: PhantomData<T>,
}

impl<T: ActionLayout> Default for RoSpriteGeneric<T> {
    fn default() -> Self {
        Self {
            animation: Handle::default(),
            action_type: ActionType::Idle,
            direction: Direction::South,
            start_time: 0,
            speed_factor: 1.0,
            fixed_duration_ms: None,
            _marker: PhantomData,
        }
    }
}

impl<T: ActionLayout> RoSpriteGeneric<T> {
    pub fn new(animation: Handle<RoAnimationAsset>) -> Self {
        Self {
            animation,
            ..Default::default()
        }
    }

    pub fn action_index(&self) -> usize {
        T::calculate_action_index(self.action_type, self.direction)
    }

    pub fn is_looping(&self) -> bool {
        T::is_looping(self.action_type)
    }

    pub fn set_action(&mut self, action_type: ActionType, game_time_ms: u32) {
        self.set_action_with_duration(action_type, None, game_time_ms);
    }

    /// Start an action, optionally stretched to a fixed duration.
    /// Re-entering the same non-looping action restarts it from the first frame,
    /// so consecutive attacks don't stay frozen on the last frame.
    pub fn set_action_with_duration(
        &mut self,
        action_type: ActionType,
        duration_ms: Option<u32>,
        game_time_ms: u32,
    ) {
        if self.action_type == action_type && T::is_looping(action_type) {
            return;
        }

        self.action_type = action_type;
        self.start_time = game_time_ms;
        self.fixed_duration_ms = duration_ms;
    }

    pub fn set_direction(&mut self, direction: Direction) {
        self.direction = direction;
    }

    pub fn get_frame<'a>(
        &self,
        animation: &'a RoAnimationAsset,
        game_time_ms: u32,
    ) -> Option<&'a FrameData> {
        let action_index = T::validate_action_index(self.action_index(), animation.actions.len());
        let action_data = animation.actions.get(action_index)?;

        if action_data.frames.is_empty() {
            return None;
        }

        let frame_index =
            self.frame_index(action_data.frames.len(), action_data.delay_ms, game_time_ms);
        action_data.frames.get(frame_index)
    }

    pub fn get_static_frame<'a>(&self, animation: &'a RoAnimationAsset) -> Option<&'a FrameData> {
        let action_index = T::validate_action_index(self.action_index(), animation.actions.len());
        let action_data = animation.actions.get(action_index)?;
        action_data.frames.first()
    }

    pub fn get_frame_index(&self, animation: &RoAnimationAsset, game_time_ms: u32) -> usize {
        let action_index = T::validate_action_index(self.action_index(), animation.actions.len());
        let Some(action_data) = animation.actions.get(action_index) else {
            return 0;
        };

        self.frame_index(action_data.frames.len(), action_data.delay_ms, game_time_ms)
    }

    fn frame_index(&self, frame_count: usize, delay_ms: f32, game_time_ms: u32) -> usize {
        if frame_count == 0 {
            return 0;
        }

        let elapsed = game_time_ms.wrapping_sub(self.start_time);
        let frame_time = match self.fixed_duration_ms {
            Some(duration) => {
                (elapsed as u64 * frame_count as u64 / u64::from(duration.max(1))) as usize
            }
            None => {
                let delay = (delay_ms * self.speed_factor).max(1.0);
                (elapsed as f32 / delay) as usize
            }
        };

        if self.is_looping() {
            frame_time % frame_count
        } else {
            frame_time.min(frame_count - 1)
        }
    }
}

// Type aliases
use crate::domain::entities::sprite_rendering::layout::{MobLayout, PlayerLayout};

pub type PlayerSprite = RoSpriteGeneric<PlayerLayout>;
pub type MobSprite = RoSpriteGeneric<MobLayout>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrigger_non_looping_action_restarts() {
        let mut sprite = PlayerSprite::default();
        sprite.set_action(ActionType::Attack, 1000);
        assert_eq!(sprite.start_time, 1000);

        sprite.set_action(ActionType::Attack, 2500);
        assert_eq!(sprite.start_time, 2500);
    }

    #[test]
    fn test_retrigger_looping_action_does_not_restart() {
        let mut sprite = PlayerSprite::default();
        sprite.set_action(ActionType::Walk, 1000);
        sprite.set_action(ActionType::Walk, 2500);
        assert_eq!(sprite.start_time, 1000);
    }

    #[test]
    fn test_fixed_duration_stretches_frames() {
        let mut sprite = PlayerSprite::default();
        sprite.set_action_with_duration(ActionType::Attack, Some(600), 0);

        assert_eq!(sprite.frame_index(6, 150.0, 0), 0);
        assert_eq!(sprite.frame_index(6, 150.0, 99), 0);
        assert_eq!(sprite.frame_index(6, 150.0, 100), 1);
        assert_eq!(sprite.frame_index(6, 150.0, 599), 5);
        assert_eq!(sprite.frame_index(6, 150.0, 600), 5);
        assert_eq!(sprite.frame_index(6, 150.0, 5000), 5);
    }

    #[test]
    fn test_natural_delay_without_fixed_duration() {
        let mut sprite = PlayerSprite::default();
        sprite.set_action(ActionType::Attack, 0);

        assert_eq!(sprite.frame_index(6, 150.0, 0), 0);
        assert_eq!(sprite.frame_index(6, 150.0, 149), 0);
        assert_eq!(sprite.frame_index(6, 150.0, 150), 1);
        assert_eq!(sprite.frame_index(6, 150.0, 5000), 5);
    }

    #[test]
    fn test_changing_action_clears_fixed_duration() {
        let mut sprite = PlayerSprite::default();
        sprite.set_action_with_duration(ActionType::Attack, Some(600), 0);
        assert_eq!(sprite.fixed_duration_ms, Some(600));

        sprite.set_action(ActionType::Idle, 700);
        assert_eq!(sprite.fixed_duration_ms, None);
    }
}
