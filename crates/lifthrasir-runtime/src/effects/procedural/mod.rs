pub mod ambient;
pub mod impact;
pub mod portal;
pub mod skill_fx;

use bevy::prelude::*;

/// Shared procedural visual phases; gameplay adapters order themselves before this set.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VfxSystems;
