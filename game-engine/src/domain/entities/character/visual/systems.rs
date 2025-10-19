use crate::domain::entities::character::components::visual::{CharacterDirection, CharacterSprite};
use bevy::prelude::*;

/// Updates the character sprite's facing direction when CharacterDirection changes.
///
/// This system monitors the `CharacterDirection` component for changes and automatically
/// updates the sprite's action index to reflect the new direction while maintaining
/// the current action type (Idle, Walk, Attack, etc.).
///
/// # Behavior
///
/// - Runs only when `CharacterDirection` has changed (using Bevy's change detection)
/// - Preserves the current animation action type (e.g., walking remains walking)
/// - Updates the action index to match the new direction
/// - Preserves animation frame and timer to avoid visual discontinuity
///
/// # System Integration
///
/// This system should run in the `Update` schedule after movement systems but before
/// rendering systems. It reacts to direction changes from:
/// - Movement input systems
/// - Network synchronization
/// - AI/NPC behavior systems
///
/// # Example Flow
///
/// 1. Movement system sets `CharacterDirection { facing: Direction::North }`
/// 2. Bevy detects the change via `Changed<CharacterDirection>`
/// 3. This system retrieves current action type (e.g., `ActionType::Walk`)
/// 4. Calls `sprite.update_direction(Direction::North, ActionType::Walk)`
/// 5. Sprite's action index changes from Walk-South (8) to Walk-North (12)
/// 6. Animation continues seamlessly at the same frame
///
/// # Performance
///
/// - Only processes entities with changed direction (O(changed entities))
/// - No allocations or heavy computations
/// - Direct component access with no hierarchy traversal
pub fn update_character_facing_on_direction_change(
    mut query: Query<(&CharacterDirection, &mut CharacterSprite), Changed<CharacterDirection>>,
) {
    for (direction, mut sprite) in query.iter_mut() {
        let current_action_type = sprite.get_current_action_type();

        let old_action_index = sprite.current_action;
        sprite.update_direction(direction.facing, current_action_type);

        if sprite.current_action != old_action_index {
            debug!(
                "Direction changed: facing={:?}, action_type={:?}, old_index={}, new_index={}",
                direction.facing, current_action_type, old_action_index, sprite.current_action
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::character::components::visual::{ActionType, Direction};

    #[test]
    fn test_direction_update_preserves_action_type() {
        let mut app = App::new();
        app.add_systems(Update, update_character_facing_on_direction_change);

        let entity = app
            .world_mut()
            .spawn((
                CharacterDirection {
                    facing: Direction::South,
                },
                CharacterSprite::default(),
            ))
            .id();

        app.update();

        let mut sprite = app.world_mut().get_mut::<CharacterSprite>(entity).unwrap();
        sprite.play_action(ActionType::Walk, Direction::South);
        assert_eq!(sprite.current_action, 8);

        let mut direction = app
            .world_mut()
            .get_mut::<CharacterDirection>(entity)
            .unwrap();
        direction.facing = Direction::North;

        app.update();

        let sprite = app.world().get::<CharacterSprite>(entity).unwrap();
        assert_eq!(sprite.current_action, 12);
        assert_eq!(sprite.get_current_action_type(), ActionType::Walk);
    }

    #[test]
    fn test_direction_update_idle_to_idle() {
        let mut app = App::new();
        app.add_systems(Update, update_character_facing_on_direction_change);

        let entity = app
            .world_mut()
            .spawn((
                CharacterDirection {
                    facing: Direction::South,
                },
                CharacterSprite::default(),
            ))
            .id();

        app.update();

        let sprite = app.world().get::<CharacterSprite>(entity).unwrap();
        assert_eq!(sprite.current_action, 0);
        assert_eq!(sprite.get_current_action_type(), ActionType::Idle);

        let mut direction = app
            .world_mut()
            .get_mut::<CharacterDirection>(entity)
            .unwrap();
        direction.facing = Direction::East;

        app.update();

        let sprite = app.world().get::<CharacterSprite>(entity).unwrap();
        assert_eq!(sprite.current_action, 6);
        assert_eq!(sprite.get_current_action_type(), ActionType::Idle);
    }

    #[test]
    fn test_no_update_when_direction_unchanged() {
        let mut app = App::new();
        app.add_systems(Update, update_character_facing_on_direction_change);

        let entity = app
            .world_mut()
            .spawn((
                CharacterDirection {
                    facing: Direction::South,
                },
                CharacterSprite::default(),
            ))
            .id();

        app.update();

        let initial_action = app
            .world()
            .get::<CharacterSprite>(entity)
            .unwrap()
            .current_action;

        app.update();

        let final_action = app
            .world()
            .get::<CharacterSprite>(entity)
            .unwrap()
            .current_action;
        assert_eq!(initial_action, final_action);
    }

    #[test]
    fn test_direction_update_during_attack() {
        let mut app = App::new();
        app.add_systems(Update, update_character_facing_on_direction_change);

        let entity = app
            .world_mut()
            .spawn((
                CharacterDirection {
                    facing: Direction::South,
                },
                CharacterSprite::default(),
            ))
            .id();

        app.update();

        let mut sprite = app.world_mut().get_mut::<CharacterSprite>(entity).unwrap();
        sprite.play_action(ActionType::Attack, Direction::South);
        assert_eq!(sprite.current_action, 32);

        let mut direction = app
            .world_mut()
            .get_mut::<CharacterDirection>(entity)
            .unwrap();
        direction.facing = Direction::NorthEast;

        app.update();

        let sprite = app.world().get::<CharacterSprite>(entity).unwrap();
        assert_eq!(sprite.current_action, 37);
        assert_eq!(sprite.get_current_action_type(), ActionType::Attack);
    }
}
