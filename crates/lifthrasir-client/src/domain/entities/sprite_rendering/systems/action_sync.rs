use lifthrasir_runtime::actors::sprites::systems::action_sync::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::character::states::AnimationState;
    use bevy::prelude::*;
    use lifthrasir_runtime::actors::sprites::components::PlayerSprite;
    use lifthrasir_runtime::actors::sprites::legacy::ActionType;
    use lifthrasir_runtime::effects::overlays::AnimationPaused;
    #[test]
    fn frozen_unit_ignores_animation_state_change() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(
            Update,
            (
                crate::presentation::actors::sync_actor_animation_inputs,
                sync_player_sprite_action,
            )
                .chain(),
        );

        let unit = app
            .world_mut()
            .spawn((
                AnimationState::Idle,
                PlayerSprite::default(),
                AnimationPaused { at_ms: 500 },
            ))
            .id();

        let before = app.world().get::<PlayerSprite>(unit).unwrap().start_time;

        // Taking damage while frozen fires a Hit state change (the common case).
        *app.world_mut().get_mut::<AnimationState>(unit).unwrap() = AnimationState::Hit;
        app.update();

        let after = app.world().get::<PlayerSprite>(unit).unwrap();
        assert_eq!(after.start_time, before, "pause must not restart the clock");
        assert_eq!(
            after.action_type,
            ActionType::Idle,
            "frozen unit must not play Hit"
        );
    }
}
