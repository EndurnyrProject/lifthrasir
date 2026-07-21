use crate::domain::entities::character::components::visual::{ActionType, Direction};

pub trait ActionLayout: Send + Sync + 'static {
    fn action_offset(action_type: ActionType) -> usize;

    fn calculate_action_index(action_type: ActionType, direction: Direction) -> usize {
        Self::action_offset(action_type) + (direction as usize)
    }

    fn is_looping(action_type: ActionType) -> bool;

    fn validate_action_index(index: usize, total_actions: usize) -> usize {
        if index >= total_actions { 0 } else { index }
    }
}
