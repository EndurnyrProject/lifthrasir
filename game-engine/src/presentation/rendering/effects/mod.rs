pub mod ambient;
pub mod cast_circle;
pub mod impact;
pub mod jupitel;
pub mod portal;
pub mod skill_fx;

use bevy::prelude::*;
use bevy_hanabi::prelude::*;

pub use ambient::MapAmbientVfxPlugin;
pub use cast_circle::CastCircleVfxPlugin;
pub use impact::ImpactVfxPlugin;
pub use portal::{PortalVfx, PortalVfxPlugin};
pub use skill_fx::SkillFxPlugin;

/// Ordering anchor for presentation-layer visual-effect systems (hanabi particle
/// attach, custom-material drivers, future effects, ...). Each effect plugin
/// schedules its `Update` systems `.in_set(VfxSystems)`.
///
/// NOTE: a single marker set for now. Promote to an enum with explicit phases if
/// effects ever need ordering between themselves.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VfxSystems;

/// Aggregate plugin for presentation-layer VFX. Owns `HanabiPlugin` once (a
/// second `add_plugins(HanabiPlugin)` panics) and registers each effect plugin.
/// Add new effects here.
pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HanabiPlugin)
            .configure_sets(Update, VfxSystems)
            .add_plugins(PortalVfxPlugin)
            .add_plugins(ImpactVfxPlugin)
            .add_plugins(SkillFxPlugin)
            .add_plugins(MapAmbientVfxPlugin)
            .add_plugins(CastCircleVfxPlugin);
    }
}
