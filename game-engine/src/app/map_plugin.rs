use crate::{
    app::map_domain_plugin::MapDomainPlugin, domain::assets::components::WaterMaterial,
    presentation::rendering::lighting::EnhancedLightingPlugin,
};
use bevy::prelude::*;

/// Map Plugin
///
/// Composes map rendering functionality with proper dependency order:
/// 1. Material plugins (infrastructure-level)
/// 2. EnhancedLightingPlugin (sub-plugin)
/// 3. MapDomainPlugin (auto-plugin with systems)
pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MaterialPlugin::<WaterMaterial>::default(),
            EnhancedLightingPlugin,
            MapDomainPlugin,
        ));

        info!("MapPlugin initialized");
    }
}
