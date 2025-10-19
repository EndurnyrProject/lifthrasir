use super::visual::{ActionType, Direction};
use bevy::prelude::*;

/// Action offsets in ACT files for standard Ragnarok Online character sprites.
///
/// Each action type has 8 directional variants (except special actions).
/// The formula to get the actual action index is: base_offset + direction
pub mod action_offsets {
    pub const IDLE: usize = 0;
    pub const WALK: usize = 8;
    pub const SIT: usize = 16;
    pub const PICKUP: usize = 24;
    pub const ATTACK: usize = 32;
    pub const HIT: usize = 40;
    pub const DEAD: usize = 48;
}

/// Calculate the action index in the ACT file based on action type and direction.
///
/// # Arguments
///
/// * `action_type` - The type of action (Idle, Walk, Sit, etc.)
/// * `direction` - The 8-directional facing (South=0, SouthWest=1, ..., SouthEast=7)
///
/// # Returns
///
/// The calculated action index to use in the ACT file
///
/// # Example
///
/// ```ignore
/// let action_index = calculate_action_index(ActionType::Walk, Direction::North);
/// // Returns 12 (WALK base 8 + North direction 4)
/// ```
pub fn calculate_action_index(action_type: ActionType, direction: Direction) -> usize {
    let base_offset = match action_type {
        ActionType::Idle => action_offsets::IDLE,
        ActionType::Walk => action_offsets::WALK,
        ActionType::Sit => action_offsets::SIT,
        ActionType::Attack => action_offsets::ATTACK,
        ActionType::Hit => action_offsets::HIT,
        ActionType::Dead => action_offsets::DEAD,
        ActionType::Cast => action_offsets::PICKUP,
        ActionType::Special => action_offsets::PICKUP,
    };

    base_offset + (direction as usize)
}

/// Validate that an action index is within bounds for the given total actions.
///
/// This function performs bounds checking and returns a safe action index.
/// If the requested index is out of bounds, it logs a warning and returns
/// action 0 (IDLE South) as a fallback.
///
/// # Arguments
///
/// * `action_index` - The action index to validate
/// * `total_actions` - The total number of actions available in the ACT file
///
/// # Returns
///
/// A valid action index (either the input or 0 as fallback)
///
/// # Example
///
/// ```ignore
/// let safe_index = validate_action_index(50, 48);
/// // Returns 0 and logs warning because 50 >= 48
/// ```
pub fn validate_action_index(action_index: usize, total_actions: usize) -> usize {
    if action_index >= total_actions {
        warn!(
            "Action index {} is out of bounds (total: {}). Falling back to action 0.",
            action_index, total_actions
        );
        return 0;
    }

    action_index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idle_action_mapping() {
        assert_eq!(
            calculate_action_index(ActionType::Idle, Direction::South),
            0
        );
        assert_eq!(
            calculate_action_index(ActionType::Idle, Direction::SouthWest),
            1
        );
        assert_eq!(calculate_action_index(ActionType::Idle, Direction::West), 2);
        assert_eq!(
            calculate_action_index(ActionType::Idle, Direction::NorthWest),
            3
        );
        assert_eq!(
            calculate_action_index(ActionType::Idle, Direction::North),
            4
        );
        assert_eq!(
            calculate_action_index(ActionType::Idle, Direction::NorthEast),
            5
        );
        assert_eq!(calculate_action_index(ActionType::Idle, Direction::East), 6);
        assert_eq!(
            calculate_action_index(ActionType::Idle, Direction::SouthEast),
            7
        );
    }

    #[test]
    fn test_walk_action_mapping() {
        assert_eq!(
            calculate_action_index(ActionType::Walk, Direction::South),
            8
        );
        assert_eq!(
            calculate_action_index(ActionType::Walk, Direction::SouthWest),
            9
        );
        assert_eq!(
            calculate_action_index(ActionType::Walk, Direction::West),
            10
        );
        assert_eq!(
            calculate_action_index(ActionType::Walk, Direction::NorthWest),
            11
        );
        assert_eq!(
            calculate_action_index(ActionType::Walk, Direction::North),
            12
        );
        assert_eq!(
            calculate_action_index(ActionType::Walk, Direction::NorthEast),
            13
        );
        assert_eq!(
            calculate_action_index(ActionType::Walk, Direction::East),
            14
        );
        assert_eq!(
            calculate_action_index(ActionType::Walk, Direction::SouthEast),
            15
        );
    }

    #[test]
    fn test_sit_action_mapping() {
        assert_eq!(
            calculate_action_index(ActionType::Sit, Direction::South),
            16
        );
        assert_eq!(
            calculate_action_index(ActionType::Sit, Direction::SouthWest),
            17
        );
        assert_eq!(calculate_action_index(ActionType::Sit, Direction::West), 18);
        assert_eq!(
            calculate_action_index(ActionType::Sit, Direction::NorthWest),
            19
        );
        assert_eq!(
            calculate_action_index(ActionType::Sit, Direction::North),
            20
        );
        assert_eq!(
            calculate_action_index(ActionType::Sit, Direction::NorthEast),
            21
        );
        assert_eq!(calculate_action_index(ActionType::Sit, Direction::East), 22);
        assert_eq!(
            calculate_action_index(ActionType::Sit, Direction::SouthEast),
            23
        );
    }

    #[test]
    fn test_attack_action_mapping() {
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::South),
            32
        );
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::SouthWest),
            33
        );
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::West),
            34
        );
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::NorthWest),
            35
        );
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::North),
            36
        );
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::NorthEast),
            37
        );
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::East),
            38
        );
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::SouthEast),
            39
        );
    }

    #[test]
    fn test_hit_action_mapping() {
        assert_eq!(
            calculate_action_index(ActionType::Hit, Direction::South),
            40
        );
        assert_eq!(
            calculate_action_index(ActionType::Hit, Direction::SouthWest),
            41
        );
        assert_eq!(calculate_action_index(ActionType::Hit, Direction::West), 42);
        assert_eq!(
            calculate_action_index(ActionType::Hit, Direction::NorthWest),
            43
        );
        assert_eq!(
            calculate_action_index(ActionType::Hit, Direction::North),
            44
        );
        assert_eq!(
            calculate_action_index(ActionType::Hit, Direction::NorthEast),
            45
        );
        assert_eq!(calculate_action_index(ActionType::Hit, Direction::East), 46);
        assert_eq!(
            calculate_action_index(ActionType::Hit, Direction::SouthEast),
            47
        );
    }

    #[test]
    fn test_dead_action_mapping() {
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::South),
            48
        );
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::SouthWest),
            49
        );
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::West),
            50
        );
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::NorthWest),
            51
        );
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::North),
            52
        );
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::NorthEast),
            53
        );
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::East),
            54
        );
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::SouthEast),
            55
        );
    }

    #[test]
    fn test_cast_action_mapping() {
        assert_eq!(
            calculate_action_index(ActionType::Cast, Direction::South),
            24
        );
        assert_eq!(
            calculate_action_index(ActionType::Cast, Direction::NorthEast),
            29
        );
    }

    #[test]
    fn test_special_action_mapping() {
        assert_eq!(
            calculate_action_index(ActionType::Special, Direction::South),
            24
        );
        assert_eq!(
            calculate_action_index(ActionType::Special, Direction::East),
            30
        );
    }

    #[test]
    fn test_validate_action_index_within_bounds() {
        assert_eq!(validate_action_index(0, 56), 0);
        assert_eq!(validate_action_index(10, 56), 10);
        assert_eq!(validate_action_index(55, 56), 55);
    }

    #[test]
    fn test_validate_action_index_out_of_bounds() {
        assert_eq!(validate_action_index(56, 56), 0);
        assert_eq!(validate_action_index(100, 56), 0);
        assert_eq!(validate_action_index(999, 56), 0);
    }

    #[test]
    fn test_validate_action_index_edge_case_zero_actions() {
        assert_eq!(validate_action_index(0, 0), 0);
        assert_eq!(validate_action_index(1, 0), 0);
    }

    #[test]
    fn test_all_directions_sequence() {
        let directions = [
            Direction::South,
            Direction::SouthWest,
            Direction::West,
            Direction::NorthWest,
            Direction::North,
            Direction::NorthEast,
            Direction::East,
            Direction::SouthEast,
        ];

        for (i, direction) in directions.iter().enumerate() {
            assert_eq!(
                calculate_action_index(ActionType::Idle, *direction),
                action_offsets::IDLE + i
            );
        }

        for (i, direction) in directions.iter().enumerate() {
            assert_eq!(
                calculate_action_index(ActionType::Walk, *direction),
                action_offsets::WALK + i
            );
        }
    }
}
