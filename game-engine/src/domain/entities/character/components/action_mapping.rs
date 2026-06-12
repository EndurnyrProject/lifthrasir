use super::visual::{ActionType, Direction};
use bevy::prelude::*;

/// Action offsets in ACT files for standard Ragnarok Online PC/humanoid sprites.
///
/// Each action type has 8 directional variants (except special actions).
/// The formula to get the actual action index is: base_offset + direction
///
/// PC sprites have a different layout than mobs:
/// - Idle = 0, Walk = 8, Sit = 16, PickUp = 24, Standby = 32
/// - Hit = 48, Freeze1 = 56, Dead = 64, Freeze2 = 72
/// - Attack2 = 80, Attack1/Attack3 = 88, Casting = 96
pub mod action_offsets {
    pub const IDLE: usize = 0; // 0 * 8
    pub const WALK: usize = 8; // 1 * 8
    pub const SIT: usize = 16; // 2 * 8
    pub const PICKUP: usize = 24; // 3 * 8
    pub const STANDBY: usize = 32; // 4 * 8 (combat idle)
    pub const HIT: usize = 48; // 6 * 8
    pub const DEAD: usize = 64; // 8 * 8
    pub const ATTACK: usize = 88; // 11 * 8 (Attack1/Attack3)
    pub const CASTING: usize = 96; // 12 * 8
}

/// Action offsets in ACT files for Ragnarok Online mob/monster sprites.
///
/// Mobs have a simpler action layout than PCs (no sit/pickup actions).
/// The formula to get the actual action index is: base_offset + direction
pub mod mob_action_offsets {
    pub const IDLE: usize = 0;
    pub const WALK: usize = 8;
    pub const ATTACK: usize = 16;
    pub const HIT: usize = 24;
    pub const DEAD: usize = 32;
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
        ActionType::Cast => action_offsets::CASTING,
        ActionType::Special => action_offsets::PICKUP,
    };

    base_offset + (direction as usize)
}

/// Calculate the action index for mob/monster sprites.
///
/// Mobs have a different action layout than PCs with fewer action types.
pub fn calculate_mob_action_index(action_type: ActionType, direction: Direction) -> usize {
    let base_offset = match action_type {
        ActionType::Idle => mob_action_offsets::IDLE,
        ActionType::Walk => mob_action_offsets::WALK,
        ActionType::Attack => mob_action_offsets::ATTACK,
        ActionType::Hit => mob_action_offsets::HIT,
        ActionType::Dead => mob_action_offsets::DEAD,
        // Mobs don't have sit/cast/special - default to idle
        ActionType::Sit | ActionType::Cast | ActionType::Special => mob_action_offsets::IDLE,
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
            88
        );
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::SouthWest),
            89
        );
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::West),
            90
        );
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::NorthWest),
            91
        );
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::North),
            92
        );
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::NorthEast),
            93
        );
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::East),
            94
        );
        assert_eq!(
            calculate_action_index(ActionType::Attack, Direction::SouthEast),
            95
        );
    }

    #[test]
    fn test_hit_action_mapping() {
        assert_eq!(
            calculate_action_index(ActionType::Hit, Direction::South),
            48
        );
        assert_eq!(
            calculate_action_index(ActionType::Hit, Direction::SouthWest),
            49
        );
        assert_eq!(calculate_action_index(ActionType::Hit, Direction::West), 50);
        assert_eq!(
            calculate_action_index(ActionType::Hit, Direction::NorthWest),
            51
        );
        assert_eq!(
            calculate_action_index(ActionType::Hit, Direction::North),
            52
        );
        assert_eq!(
            calculate_action_index(ActionType::Hit, Direction::NorthEast),
            53
        );
        assert_eq!(calculate_action_index(ActionType::Hit, Direction::East), 54);
        assert_eq!(
            calculate_action_index(ActionType::Hit, Direction::SouthEast),
            55
        );
    }

    #[test]
    fn test_dead_action_mapping() {
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::South),
            64
        );
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::SouthWest),
            65
        );
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::West),
            66
        );
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::NorthWest),
            67
        );
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::North),
            68
        );
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::NorthEast),
            69
        );
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::East),
            70
        );
        assert_eq!(
            calculate_action_index(ActionType::Dead, Direction::SouthEast),
            71
        );
    }

    #[test]
    fn test_cast_action_mapping() {
        assert_eq!(
            calculate_action_index(ActionType::Cast, Direction::South),
            96
        );
        assert_eq!(
            calculate_action_index(ActionType::Cast, Direction::NorthEast),
            101
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
