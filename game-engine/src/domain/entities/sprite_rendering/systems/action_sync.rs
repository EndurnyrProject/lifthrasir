use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::domain::combat::components::AttackTimer;
use crate::domain::entities::character::components::visual::{ActionType, CharacterDirection};
use crate::domain::entities::character::states::AnimationState;
use crate::domain::entities::sprite_rendering::components::{
    MobSprite, PlayerSprite, RoSpriteGeneric,
};
use crate::domain::entities::sprite_rendering::layout::ActionLayout;
use crate::domain::system_sets::SpriteRenderingSystems;

fn sync_sprite_action_impl<T: ActionLayout>(
    time: &Res<Time>,
    query: &mut Query<
        (
            &AnimationState,
            Option<&AttackTimer>,
            &mut RoSpriteGeneric<T>,
        ),
        Or<(Changed<AnimationState>, Added<RoSpriteGeneric<T>>)>,
    >,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;

    for (state, attack_timer, mut ro_sprite) in query.iter_mut() {
        let action_type: ActionType = (*state).into();
        let duration_ms = attack_timer
            .filter(|_| action_type == ActionType::Attack)
            .map(|timer| timer.timer.duration().as_millis() as u32);
        ro_sprite.set_action_with_duration(action_type, duration_ms, game_time_ms);
    }
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
pub fn sync_player_sprite_action(
    time: Res<Time>,
    mut query: Query<
        (&AnimationState, Option<&AttackTimer>, &mut PlayerSprite),
        Or<(Changed<AnimationState>, Added<PlayerSprite>)>,
    >,
) {
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
pub fn sync_mob_sprite_action(
    time: Res<Time>,
    mut query: Query<
        (&AnimationState, Option<&AttackTimer>, &mut MobSprite),
        Or<(Changed<AnimationState>, Added<MobSprite>)>,
    >,
) {
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
