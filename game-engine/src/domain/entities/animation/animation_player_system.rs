use super::animation_player::RoAnimationPlayer;
use super::markers::Animated;
use super::state::AnimationState;
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use bevy::prelude::*;

type AnimatedEntitiesQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut RoAnimationPlayer,
        &'static mut Sprite,
        &'static mut Transform,
        Option<&'static AnimationState>,
        Option<&'static ViewVisibility>,
    ),
    With<Animated>,
>;

/// Optimized animation system using Bevy ECS patterns:
/// - Sparse-set query with Animated marker for efficient iteration
/// - Visibility culling to skip off-screen entities
/// - AnimationState control for pause/play mechanics
/// - Image handle swapping with zero runtime RGBA conversions
pub fn ro_animation_player_system(
    time: Res<Time>,
    animation_assets: Res<Assets<RoAnimationAsset>>,
    mut query: AnimatedEntitiesQuery,
) {
    for (mut player, mut sprite, mut transform, state, visibility) in query.iter_mut() {
        if let Some(vis) = visibility {
            if !vis.get() {
                continue;
            }
        }

        let state = state.copied().unwrap_or(AnimationState::Playing);
        match state {
            AnimationState::Paused | AnimationState::Finished | AnimationState::Waiting => {
                continue;
            }
            AnimationState::Playing => {}
        }

        if player.paused {
            continue;
        }

        player.timer.tick(time.delta());

        if !player.timer.just_finished() {
            continue;
        }

        let Some(animation) = animation_assets.get(&player.animation) else {
            continue;
        };

        if animation.frames.is_empty() {
            continue;
        }

        let next_index = if player.loop_animation {
            (player.frame_index + 1) % animation.frames.len()
        } else {
            (player.frame_index + 1).min(animation.frames.len() - 1)
        };

        if next_index != player.frame_index {
            player.frame_index = next_index;

            sprite.image = animation.frames[player.frame_index].clone();

            if let Some(offset) = animation.frame_offsets.get(player.frame_index) {
                transform.translation.x = offset.0;
                transform.translation.y = offset.1;
            }
        }

        if player.timer.duration() != animation.frame_duration {
            player.timer = Timer::new(animation.frame_duration, TimerMode::Repeating);
        }
    }
}
