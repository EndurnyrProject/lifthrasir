use crate::{core::MapState, plugins::world_domain_plugin::WorldDomainPlugin};
use bevy::prelude::*;

/// World Plugin (Wrapper)
///
/// Minimal wrapper that handles state initialization (cannot be auto-derived).
/// All systems and resources are managed by WorldDomainPlugin.
pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<MapState>();
        app.add_plugins(WorldDomainPlugin);

        info!("WorldPlugin initialized");
    }
}
