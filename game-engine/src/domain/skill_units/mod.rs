//! Server-authoritative ground-skill unit entities (Storm Gust, Ice Wall, ...).
//!
//! A `SkillUnitGroup` root owns `SkillUnitCell` children; both are driven purely
//! by the four skill-unit lifecycle messages and carry no client-side timers.
//! `spawn` builds the group/cell hierarchy and click colliders, `visuals`
//! attaches the persistent per-group or per-cell visual, `lifecycle` applies
//! server HP updates and despawns.

pub mod components;
mod lifecycle;
mod spawn;
mod visuals;

#[cfg(test)]
mod tests;

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
                spawn::spawn_skill_units,
                lifecycle::update_skill_units,
                lifecycle::despawn_skill_units,
            ),
        );
    }
}
