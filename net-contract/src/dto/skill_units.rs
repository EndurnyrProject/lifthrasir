//! Protocol-neutral skill-unit (ground skill) group/cell types.

/// Decoded per-cell wire flags. Bit-position decoding is aesir wire detail
/// and stays in `net-aesir`; the contract only carries the typed result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkillUnitCellFlags {
    pub targetable: bool,
    pub blocks_movement: bool,
    pub blocks_projectiles: bool,
    pub consumable_water: bool,
    pub visible: bool,
}

/// One cell of a skill-unit group (e.g. one Ice Wall tile, one Storm Gust tile).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillUnitCellState {
    pub cell_id: u32,
    pub x: i32,
    pub y: i32,
    pub hp: u32,
    pub max_hp: u32,
    pub flags: SkillUnitCellFlags,
}

/// A server-authoritative skill-unit group (e.g. one Storm Gust cast, one Ice Wall).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillUnitGroupState {
    pub group_id: u64,
    pub skill_id: u32,
    pub skill_level: u32,
    pub owner_id: u32,
    pub center_x: i32,
    pub center_y: i32,
    pub cells: Vec<SkillUnitCellState>,
}

/// Mirrors aesir's `SkillUnitDespawnReason` proto enum, neutrally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkillUnitDespawnReason {
    #[default]
    Unspecified,
    Expired,
    Destroyed,
    SourceConsumed,
    Lifecycle,
    MapShutdown,
    LeftView,
    Canceled,
}

/// Why a `SkillUnitUpdated` was emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkillUnitUpdateReason {
    #[default]
    Unspecified,
    Damage,
    Decay,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_flags_default_to_all_false() {
        assert_eq!(
            SkillUnitCellFlags::default(),
            SkillUnitCellFlags {
                targetable: false,
                blocks_movement: false,
                blocks_projectiles: false,
                consumable_water: false,
                visible: false,
            }
        );
    }

    #[test]
    fn despawn_reason_defaults_to_unspecified() {
        assert_eq!(
            SkillUnitDespawnReason::default(),
            SkillUnitDespawnReason::Unspecified
        );
    }

    #[test]
    fn update_reason_defaults_to_unspecified() {
        assert_eq!(
            SkillUnitUpdateReason::default(),
            SkillUnitUpdateReason::Unspecified
        );
    }

    #[test]
    fn group_state_round_trips_through_clone_and_equality() {
        let group = SkillUnitGroupState {
            group_id: 1,
            skill_id: 89,
            skill_level: 10,
            owner_id: 42,
            center_x: 150,
            center_y: 150,
            cells: vec![SkillUnitCellState {
                cell_id: 100,
                x: 150,
                y: 150,
                hp: 100,
                max_hp: 100,
                flags: SkillUnitCellFlags {
                    targetable: true,
                    ..Default::default()
                },
            }],
        };

        let cloned = group.clone();

        assert_eq!(group, cloned);
    }

    #[test]
    fn group_state_supports_negative_coordinates() {
        let group = SkillUnitGroupState {
            group_id: u64::from(u32::MAX) + 1,
            skill_id: 89,
            skill_level: 10,
            owner_id: 42,
            center_x: -1,
            center_y: -1,
            cells: vec![],
        };

        assert_eq!(group.center_x, -1);
        assert_eq!(group.center_y, -1);
        assert_eq!(group.group_id, u64::from(u32::MAX) + 1);
    }
}
