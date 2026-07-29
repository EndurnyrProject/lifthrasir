use crate::presentation::rendering::water::WaterMaterial;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

/// Map Domain Plugin
///
/// Handles map model rendering, RSM assets, and water systems. Systems
/// register themselves via `auto_*` attributes in `presentation/rendering/*`
/// and `domain/effects/map_effects.rs`.
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct MapDomainPlugin;

/// Map Plugin
///
/// Composes map rendering functionality with proper dependency order:
/// 1. Material plugins (infrastructure-level)
/// 2. MapDomainPlugin (auto-plugin with systems)
pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((MaterialPlugin::<WaterMaterial>::default(), MapDomainPlugin));
    }
}
