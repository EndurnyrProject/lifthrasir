use crate::core::MapState;
use crate::domain::world::loading_progress::MapLoadProgressPlugin;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

/// World Domain Plugin
///
/// Handles world loading, terrain generation, and map state management.
/// Systems and resources register themselves via `auto_*` attributes in
/// `domain/world/*` and `core/resources.rs`.
#[derive(AutoPlugin)]
pub struct WorldDomainPlugin;

impl Plugin for WorldDomainPlugin {
    #[auto_plugin]
    fn build(&self, app: &mut App) {
        app.init_state::<MapState>();
        app.add_plugins(MapLoadProgressPlugin);
    }
}
