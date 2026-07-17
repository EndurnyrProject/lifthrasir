//! Server-authoritative ground-skill unit entities (Storm Gust, Ice Wall, ...).
//!
//! A `SkillUnitGroup` root owns `SkillUnitCell` children; both are driven purely
//! by the four skill-unit lifecycle messages and carry no client-side timers.

pub mod components;
pub mod systems;

pub use components::{SkillUnitCell, SkillUnitGroup};

use bevy::prelude::*;

/// Registers the skill-unit lifecycle systems. The lifecycle messages themselves
/// are registered by `NetContractPlugin`.
pub struct SkillUnitsPlugin;

impl Plugin for SkillUnitsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                systems::spawn_skill_units,
                systems::update_skill_units,
                systems::despawn_skill_units,
            ),
        );
    }
}
