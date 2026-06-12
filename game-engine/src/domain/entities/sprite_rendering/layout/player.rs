use super::trait_def::ActionLayout;
use crate::domain::entities::character::components::visual::ActionType;

pub struct PlayerLayout;

impl ActionLayout for PlayerLayout {
    fn action_offset(action_type: ActionType) -> usize {
        match action_type {
            ActionType::Idle => 0,
            ActionType::Walk => 8,
            ActionType::Sit => 16,
            ActionType::Special => 24,
            ActionType::Hit => 48,
            ActionType::Dead => 64,
            ActionType::Attack => 88,
            ActionType::Cast => 96,
        }
    }

    fn is_looping(action_type: ActionType) -> bool {
        matches!(
            action_type,
            ActionType::Idle | ActionType::Walk | ActionType::Sit
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::character::components::visual::Direction;

    #[test]
    fn test_idle_directions() {
        assert_eq!(
            PlayerLayout::calculate_action_index(ActionType::Idle, Direction::South),
            0
        );
        assert_eq!(
            PlayerLayout::calculate_action_index(ActionType::Idle, Direction::North),
            4
        );
        assert_eq!(
            PlayerLayout::calculate_action_index(ActionType::Idle, Direction::SouthEast),
            7
        );
    }

    #[test]
    fn test_walk_directions() {
        assert_eq!(
            PlayerLayout::calculate_action_index(ActionType::Walk, Direction::South),
            8
        );
        assert_eq!(
            PlayerLayout::calculate_action_index(ActionType::Walk, Direction::North),
            12
        );
    }

    #[test]
    fn test_attack_directions() {
        assert_eq!(
            PlayerLayout::calculate_action_index(ActionType::Attack, Direction::South),
            88
        );
        assert_eq!(
            PlayerLayout::calculate_action_index(ActionType::Attack, Direction::East),
            94
        );
    }

    #[test]
    fn test_hit_directions() {
        assert_eq!(
            PlayerLayout::calculate_action_index(ActionType::Hit, Direction::South),
            48
        );
    }

    #[test]
    fn test_dead_directions() {
        assert_eq!(
            PlayerLayout::calculate_action_index(ActionType::Dead, Direction::South),
            64
        );
    }

    #[test]
    fn test_looping_actions() {
        assert!(PlayerLayout::is_looping(ActionType::Idle));
        assert!(PlayerLayout::is_looping(ActionType::Walk));
        assert!(PlayerLayout::is_looping(ActionType::Sit));
        assert!(!PlayerLayout::is_looping(ActionType::Attack));
        assert!(!PlayerLayout::is_looping(ActionType::Hit));
        assert!(!PlayerLayout::is_looping(ActionType::Dead));
    }

    #[test]
    fn test_validate_action_index() {
        assert_eq!(PlayerLayout::validate_action_index(10, 100), 10);
        assert_eq!(PlayerLayout::validate_action_index(100, 50), 0);
        assert_eq!(PlayerLayout::validate_action_index(0, 0), 0);
    }
}
