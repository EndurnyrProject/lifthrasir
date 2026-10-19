pub mod aoe_preview;
pub mod cast_circle;
pub mod navigation_path;
pub mod portal;

use bevy::prelude::*;

pub use crate::presentation::rendering::effects::aoe_preview::AoePreviewPlugin;
pub use crate::presentation::rendering::effects::cast_circle::CastCircleVfxPlugin;
pub use crate::presentation::rendering::effects::navigation_path::NavigationPathPlugin;

/// Ordering anchor for presentation-layer visual-effect systems (hanabi particle
/// attach, custom-material drivers, future effects, ...). Each effect plugin
/// schedules its `Update` systems `.in_set(VfxSystems)`.
///
/// NOTE: a single marker set for now. Promote to an enum with explicit phases if
/// effects ever need ordering between themselves.
use lifthrasir_runtime::effects::procedural::VfxSystems;

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
