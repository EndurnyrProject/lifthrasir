pub mod aoe_preview;
pub mod cast_circle;
mod grimtooth;
pub mod navigation_path;
mod warp_zone;

use bevy::prelude::*;

pub use crate::presentation::rendering::effects::aoe_preview::AoePreviewPlugin;
pub use crate::presentation::rendering::effects::cast_circle::CastCircleVfxPlugin;
pub use crate::presentation::rendering::effects::navigation_path::NavigationPathPlugin;

/// Client casting, targeting, navigation, and map warp effects.
/// Shared procedural playback and Hanabi are owned by EffectsPlaybackPlugin.
pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CastCircleVfxPlugin)
            .add_plugins(grimtooth::GrimtoothVfxPlugin)
            .add_plugins(AoePreviewPlugin)
            .add_plugins(NavigationPathPlugin)
            .add_plugins(warp_zone::WarpZonePlugin);
    }
}
