use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::domain::combat::components::AttackTimer;
use crate::domain::entities::character::components::visual::{ActionType, CharacterDirection};
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

fn sync_sprite_direction_impl<T: ActionLayout>(
    query: &mut Query<(&CharacterDirection, &mut RoSpriteGeneric<T>), Changed<CharacterDirection>>,
) {
    for (direction, mut ro_sprite) in query.iter_mut() {
        ro_sprite.set_direction(direction.facing);
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
    mut query: Query<(&CharacterDirection, &mut PlayerSprite), Changed<CharacterDirection>>,
) {
    sync_sprite_direction_impl(&mut query);
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
    mut query: Query<(&CharacterDirection, &mut MobSprite), Changed<CharacterDirection>>,
) {
    sync_sprite_direction_impl(&mut query);
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
}
