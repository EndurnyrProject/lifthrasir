use super::trait_def::ActionLayout;
use crate::domain::entities::character::components::visual::ActionType;

pub struct MobLayout;

impl ActionLayout for MobLayout {
    fn action_offset(action_type: ActionType) -> usize {
        match action_type {
            ActionType::Idle => 0,
            ActionType::Walk => 8,
            ActionType::Attack => 16,
            ActionType::Hit => 24,
            ActionType::Dead => 32,
            ActionType::Sit | ActionType::Cast | ActionType::Special => 0,
        }
    }

    fn is_looping(action_type: ActionType) -> bool {
        matches!(action_type, ActionType::Idle | ActionType::Walk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::character::components::visual::Direction;

    #[test]
    fn test_mob_idle() {
        assert_eq!(
            MobLayout::calculate_action_index(ActionType::Idle, Direction::South),
            0
        );
        assert_eq!(
            MobLayout::calculate_action_index(ActionType::Idle, Direction::North),
            4
        );
    }

    #[test]
    fn test_mob_attack() {
        assert_eq!(
            MobLayout::calculate_action_index(ActionType::Attack, Direction::South),
            16
        );
        assert_eq!(
            MobLayout::calculate_action_index(ActionType::Attack, Direction::East),
            22
        );
    }

    #[test]
    fn test_mob_unsupported_actions_fallback_to_idle() {
        assert_eq!(MobLayout::action_offset(ActionType::Sit), 0);
        assert_eq!(MobLayout::action_offset(ActionType::Cast), 0);
    }

    #[test]
    fn test_mob_looping() {
        assert!(MobLayout::is_looping(ActionType::Idle));
        assert!(MobLayout::is_looping(ActionType::Walk));
        assert!(!MobLayout::is_looping(ActionType::Attack));
        assert!(!MobLayout::is_looping(ActionType::Dead));
    }
}
