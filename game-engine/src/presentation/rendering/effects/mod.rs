pub mod ambient;
pub mod aoe_preview;
pub mod cast_circle;
pub mod impact;
pub mod navigation_path;
pub mod portal;
pub mod skill_fx;

use bevy::prelude::*;

pub use ambient::MapAmbientVfxPlugin;
pub use aoe_preview::AoePreviewPlugin;
pub use cast_circle::CastCircleVfxPlugin;
pub use impact::ImpactVfxPlugin;
pub use navigation_path::NavigationPathPlugin;
pub use portal::{PortalVfx, PortalVfxPlugin};
pub use skill_fx::SkillFxPlugin;

/// Ordering anchor for presentation-layer visual-effect systems (hanabi particle
/// attach, custom-material drivers, future effects, ...). Each effect plugin
/// schedules its `Update` systems `.in_set(VfxSystems)`.
///
/// NOTE: a single marker set for now. Promote to an enum with explicit phases if
/// effects ever need ordering between themselves.
pub use lifthrasir_runtime::effects::procedural::VfxSystems;

/// Client casting, targeting, navigation, and warp-portal bridges.
/// Shared procedural playback and Hanabi are owned by EffectsPlaybackPlugin.
pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, portal::attach_warp_portal_vfx.before(VfxSystems))
            .add_plugins(CastCircleVfxPlugin)
            .add_plugins(AoePreviewPlugin)
            .add_plugins(NavigationPathPlugin);
    }
}
