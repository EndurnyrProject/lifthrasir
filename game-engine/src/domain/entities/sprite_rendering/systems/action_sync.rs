use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::domain::combat::components::AttackTimer;
use crate::domain::entities::character::components::visual::{
    ActionType, CharacterDirection, Direction,
};
use crate::domain::entities::character::states::AnimationState;
use crate::domain::entities::movement::components::MovementSpeed;
use crate::domain::entities::sprite_rendering::components::{
    MobSprite, PlayerSprite, RoSpriteGeneric,
};
use crate::domain::entities::sprite_rendering::layout::{ActionLayout, MobLayout, PlayerLayout};
use crate::domain::system_sets::SpriteRenderingSystems;

/// RO's reference walk speed: one cell every 150ms. Walk animation cadence is
/// calibrated against it so units at this pace play the ACT's natural delay.
const STANDARD_WALK_MS_PER_CELL: f32 = 150.0;

type SpriteActionQuery<'w, 's, T> = Query<
    'w,
    's,
    (
        &'static AnimationState,
        Option<&'static AttackTimer>,
        Option<&'static MovementSpeed>,
        &'static mut RoSpriteGeneric<T>,
    ),
    Or<(Changed<AnimationState>, Added<RoSpriteGeneric<T>>)>,
>;

fn sync_sprite_action_impl<T: ActionLayout>(time: &Res<Time>, query: &mut SpriteActionQuery<T>) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;

    for (state, attack_timer, movement_speed, mut ro_sprite) in query.iter_mut() {
        let action_type: ActionType = (*state).into();
        let duration_ms = attack_timer
            .filter(|_| action_type == ActionType::Attack)
            .map(|timer| timer.timer.duration().as_millis() as u32);
        ro_sprite.speed_factor = walk_speed_factor(action_type, movement_speed);
        ro_sprite.set_action_with_duration(action_type, duration_ms, game_time_ms);
    }
}

/// Stretch the looping walk animation in proportion to movement speed so a slow
/// unit doesn't replay its walk cycle several times while crawling across one
/// cell. Slower-than-standard units (most mobs) get a factor > 1 (longer per-frame
/// delay); standard 150ms/cell units keep the ACT's natural rate. Non-walk actions
/// always play at their natural rate.
fn walk_speed_factor(action_type: ActionType, movement_speed: Option<&MovementSpeed>) -> f32 {
    if action_type != ActionType::Walk {
        return 1.0;
    }
    movement_speed.map_or(1.0, |speed| speed.ms_per_cell / STANDARD_WALK_MS_PER_CELL)
}

/// Quantize the camera's horizontal heading into one of the 8 sprite octants.
///
/// RO sprites are camera-relative billboards: the displayed facing is the
/// entity's world facing rotated by the camera's orientation, so a unit keeps
/// the same on-screen orientation as the camera orbits. `camera_forward` is the
/// camera's look direction (`Transform::forward`); the heading is anchored so
/// the default camera, which looks toward +Z (north), yields 0 and leaves the
/// world facing unchanged.
fn camera_view_octant(camera_forward: Vec3) -> u8 {
    let heading = camera_forward.z.atan2(camera_forward.x) - FRAC_PI_2;
    ((heading / FRAC_PI_4).round() as i32).rem_euclid(8) as u8
}

/// Rotate a world-space facing by the camera octant to pick the sprite frame to
/// display, mirroring the reference client's `(camera_direction + direction) & 7`.
fn camera_relative_direction(facing: Direction, camera_octant: u8) -> Direction {
    Direction::from_u8((facing as u8 + camera_octant) % 8)
}

/// Pick each sprite's directional frame relative to the current camera angle.
///
/// Runs every frame (not gated on `Changed<CharacterDirection>`) because the
/// displayed frame also depends on the camera, which can orbit while a unit
/// stands still. The compare-guard keeps change detection clean so the frame
/// only re-publishes when it actually changes.
fn sync_sprite_direction_impl<T: ActionLayout>(
    camera_octant: u8,
    query: &mut Query<(&CharacterDirection, &mut RoSpriteGeneric<T>)>,
) {
    for (direction, mut ro_sprite) in query.iter_mut() {
        let display = camera_relative_direction(direction.facing, camera_octant);
        if ro_sprite.direction != display {
            ro_sprite.set_direction(display);
        }
    }
}

#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AnimationSync)
)]
pub fn sync_player_sprite_action(time: Res<Time>, mut query: SpriteActionQuery<PlayerLayout>) {
    sync_sprite_action_impl(&time, &mut query);
}

#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AnimationSync, after = sync_player_sprite_action)
)]
pub fn sync_player_sprite_direction(
    camera_query: Query<&Transform, With<Camera3d>>,
    mut query: Query<(&CharacterDirection, &mut PlayerSprite)>,
) {
    let octant = camera_query
        .single()
        .map(|t| camera_view_octant(*t.forward()))
        .unwrap_or(0);
    sync_sprite_direction_impl(octant, &mut query);
}

#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AnimationSync)
)]
pub fn sync_mob_sprite_action(time: Res<Time>, mut query: SpriteActionQuery<MobLayout>) {
    sync_sprite_action_impl(&time, &mut query);
}

#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AnimationSync, after = sync_mob_sprite_action)
)]
pub fn sync_mob_sprite_direction(
    camera_query: Query<&Transform, With<Camera3d>>,
    mut query: Query<(&CharacterDirection, &mut MobSprite)>,
) {
    let octant = camera_query
        .single()
        .map(|t| camera_view_octant(*t.forward()))
        .unwrap_or(0);
    sync_sprite_direction_impl(octant, &mut query);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_speed_keeps_natural_walk_rate() {
        let speed = MovementSpeed::from_server_speed(150);
        assert_eq!(walk_speed_factor(ActionType::Walk, Some(&speed)), 1.0);
    }

    #[test]
    fn slow_mob_stretches_walk_cycle() {
        let speed = MovementSpeed::from_server_speed(450);
        assert_eq!(walk_speed_factor(ActionType::Walk, Some(&speed)), 3.0);
    }

    #[test]
    fn non_walk_actions_play_at_natural_rate() {
        let speed = MovementSpeed::from_server_speed(450);
        assert_eq!(walk_speed_factor(ActionType::Idle, Some(&speed)), 1.0);
        assert_eq!(walk_speed_factor(ActionType::Attack, Some(&speed)), 1.0);
    }

    #[test]
    fn missing_speed_defaults_to_natural_rate() {
        assert_eq!(walk_speed_factor(ActionType::Walk, None), 1.0);
    }

    // Camera forwards for the RO-style camera (looks down at the player). Only
    // the horizontal (x, z) components drive the octant; y is the downward tilt.
    const FORWARD_LOOKING_NORTH: Vec3 = Vec3::new(0.0, 0.707, 0.707); // default yaw
    const FORWARD_LOOKING_WEST: Vec3 = Vec3::new(-0.707, 0.707, 0.0); // orbited +90°
    const FORWARD_LOOKING_SOUTH: Vec3 = Vec3::new(0.0, 0.707, -0.707); // orbited 180°
    const FORWARD_LOOKING_EAST: Vec3 = Vec3::new(0.707, 0.707, 0.0); // orbited -90°

    #[test]
    fn default_camera_octant_is_identity() {
        assert_eq!(camera_view_octant(FORWARD_LOOKING_NORTH), 0);
    }

    #[test]
    fn quarter_turn_camera_octants() {
        assert_eq!(camera_view_octant(FORWARD_LOOKING_WEST), 2);
        assert_eq!(camera_view_octant(FORWARD_LOOKING_SOUTH), 4);
        assert_eq!(camera_view_octant(FORWARD_LOOKING_EAST), 6);
    }

    #[test]
    fn default_camera_shows_world_facing() {
        assert_eq!(
            camera_relative_direction(Direction::South, 0),
            Direction::South
        );
        assert_eq!(
            camera_relative_direction(Direction::East, 0),
            Direction::East
        );
    }

    #[test]
    fn facing_the_camera_renders_front() {
        // Whatever way the camera orbits, a unit facing toward it shows South
        // (front) and a unit facing away shows North (back).
        let octant = camera_view_octant(FORWARD_LOOKING_WEST); // camera on the east
        assert_eq!(
            camera_relative_direction(Direction::East, octant),
            Direction::South
        );
        assert_eq!(
            camera_relative_direction(Direction::West, octant),
            Direction::North
        );
    }

    #[test]
    fn orbiting_rotates_a_standing_units_frame() {
        // A south-facing unit seen from a 90°-orbited camera shows a side frame,
        // and from behind shows its back.
        assert_eq!(
            camera_relative_direction(Direction::South, 2),
            Direction::West
        );
        assert_eq!(
            camera_relative_direction(Direction::South, 4),
            Direction::North
        );
    }
}
