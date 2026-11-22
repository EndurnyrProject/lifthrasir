use super::trait_def::ActionLayout;
use crate::actors::ActionType;

/// Native Homunculus ACT layout: eight action groups of eight facings, matching
/// the reference client's `TYPE_HOM` table. `Attack1`/`Attack2` select the RO
/// `ATTACK2`/`ATTACK3` groups; actions with no Homunculus group stand idle.
pub struct HomunculusLayout;

impl ActionLayout for HomunculusLayout {
    fn action_offset(action_type: ActionType) -> usize {
        match action_type {
            ActionType::Idle => 0,
            ActionType::Walk => 8,
            ActionType::Attack => 16,
            ActionType::Hit => 24,
            ActionType::Dead => 32,
            ActionType::Attack1 => 40,
            ActionType::Attack2 => 48,
            ActionType::Special => 56,
            ActionType::Sit | ActionType::Cast | ActionType::ReadyFight => 0,
        }
    }

    fn is_looping(action_type: ActionType) -> bool {
        matches!(action_type, ActionType::Idle | ActionType::Walk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::Direction;

    #[test]
    fn homunculus_layout_has_native_directional_groups() {
        let groups = [
            (ActionType::Idle, 0),
            (ActionType::Walk, 1),
            (ActionType::Attack, 2),
            (ActionType::Hit, 3),
            (ActionType::Dead, 4),
            (ActionType::Attack1, 5),
            (ActionType::Attack2, 6),
            (ActionType::Special, 7),
        ];
        for (action, group) in groups {
            assert_eq!(
                HomunculusLayout::calculate_action_index(action, Direction::South),
                group * 8,
                "{action:?}"
            );
            assert_eq!(
                HomunculusLayout::calculate_action_index(action, Direction::East),
                group * 8 + 6,
                "{action:?}"
            );
        }
        for action in [ActionType::Sit, ActionType::Cast, ActionType::ReadyFight] {
            assert_eq!(HomunculusLayout::action_offset(action), 0, "{action:?}");
        }
        assert!(HomunculusLayout::is_looping(ActionType::Idle));
        assert!(HomunculusLayout::is_looping(ActionType::Walk));
        assert!(!HomunculusLayout::is_looping(ActionType::Attack1));
        assert!(!HomunculusLayout::is_looping(ActionType::Dead));
    }
}
