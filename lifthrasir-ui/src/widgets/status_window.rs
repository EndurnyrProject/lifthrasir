//! Status window: local staging model for spending status points.
//!
//! Pure UI-side logic. The client replicates only the stat-point cost curve as a
//! UX estimate (`stat_point_cost`); the server stays authoritative and reconciles
//! on Save. Combat-stat formulas are deliberately not replicated.

use bevy::prelude::*;
use game_engine::domain::entities::character::components::status::StatusParameter;
use std::collections::HashMap;

pub const PRIMARY_STATS: [StatusParameter; 6] = [
    StatusParameter::Str,
    StatusParameter::Agi,
    StatusParameter::Vit,
    StatusParameter::Int,
    StatusParameter::Dex,
    StatusParameter::Luk,
];

const STAT_CAP: u32 = 99;

/// Points staged per primary stat this session (added on top of the server base).
#[derive(Resource, Default)]
pub struct StatStaging {
    staged: HashMap<StatusParameter, u32>,
}

/// Renewal stat-point cost to raise a stat from `value` to `value + 1`.
pub fn stat_point_cost(value: u32) -> u32 {
    if value < 100 {
        2 + (value - 1) / 10
    } else {
        16 + 4 * ((value - 100) / 5)
    }
}

impl StatStaging {
    pub fn staged_value(&self, stat: StatusParameter) -> u32 {
        self.staged.get(&stat).copied().unwrap_or(0)
    }

    /// Total points spent across all staged stats, given each stat's server base.
    pub fn spent(&self, base: &HashMap<StatusParameter, u32>) -> u32 {
        self.staged
            .iter()
            .flat_map(|(stat, &staged)| {
                let start = base.get(stat).copied().unwrap_or(0);
                (0..staged).map(move |i| stat_point_cost(start + i))
            })
            .sum()
    }

    pub fn points_left(&self, status_point: u32, base: &HashMap<StatusParameter, u32>) -> u32 {
        status_point.saturating_sub(self.spent(base))
    }

    pub fn raise(
        &mut self,
        stat: StatusParameter,
        status_point: u32,
        bases: &HashMap<StatusParameter, u32>,
    ) {
        let staged = self.staged_value(stat);
        let base = bases.get(&stat).copied().unwrap_or(0);
        if can_raise(base, staged, self.points_left(status_point, bases)) {
            self.staged.insert(stat, staged + 1);
        }
    }

    pub fn lower(&mut self, stat: StatusParameter) {
        let staged = self.staged_value(stat);
        if staged > 0 {
            self.staged.insert(stat, staged - 1);
        }
    }

    pub fn clear(&mut self) {
        self.staged.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.staged.values().all(|&v| v == 0)
    }
}

/// Whether one more point can be staged: below cap and affordable.
pub fn can_raise(base: u32, staged: u32, points_left: u32) -> bool {
    let current = base + staged;
    current < STAT_CAP && stat_point_cost(current) <= points_left
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_curve_boundaries() {
        assert_eq!(stat_point_cost(1), 2);
        assert_eq!(stat_point_cost(9), 2);
        assert_eq!(stat_point_cost(10), 2);
        assert_eq!(stat_point_cost(11), 3);
        assert_eq!(stat_point_cost(99), 11);
        assert_eq!(stat_point_cost(100), 16);
        assert_eq!(stat_point_cost(104), 16);
        assert_eq!(stat_point_cost(105), 20);
    }

    #[test]
    fn points_left_after_staging_multiple_stats() {
        let bases = HashMap::from([(StatusParameter::Str, 9), (StatusParameter::Agi, 11)]);
        let mut staging = StatStaging::default();
        staging.raise(StatusParameter::Str, 100, &bases);
        staging.raise(StatusParameter::Agi, 100, &bases);

        assert_eq!(staging.staged_value(StatusParameter::Str), 1);
        assert_eq!(staging.staged_value(StatusParameter::Agi), 1);
        // cost(9) + cost(11) = 2 + 3 = 5 spent
        assert_eq!(staging.points_left(100, &bases), 95);
    }

    #[test]
    fn cannot_raise_at_cap() {
        assert!(!can_raise(99, 0, u32::MAX));
        assert!(!can_raise(98, 1, u32::MAX));
        assert!(can_raise(98, 0, u32::MAX));
    }

    #[test]
    fn cannot_raise_without_points() {
        // cost(50) = 2 + 49/10 = 6
        assert!(!can_raise(50, 0, 5));
        assert!(can_raise(50, 0, 6));
    }

    #[test]
    fn raise_blocked_when_points_left_below_next_cost() {
        let bases = HashMap::from([(StatusParameter::Str, 50)]);
        let mut staging = StatStaging::default();
        staging.raise(StatusParameter::Str, 5, &bases);
        assert_eq!(staging.staged_value(StatusParameter::Str), 0);
    }

    #[test]
    fn lower_clamped_at_base() {
        let mut staging = StatStaging::default();
        staging.lower(StatusParameter::Str);
        assert_eq!(staging.staged_value(StatusParameter::Str), 0);

        let bases = HashMap::from([(StatusParameter::Str, 1)]);
        staging.raise(StatusParameter::Str, 100, &bases);
        staging.raise(StatusParameter::Str, 100, &bases);
        assert_eq!(staging.staged_value(StatusParameter::Str), 2);
        staging.lower(StatusParameter::Str);
        staging.lower(StatusParameter::Str);
        staging.lower(StatusParameter::Str);
        assert_eq!(staging.staged_value(StatusParameter::Str), 0);
    }
}
