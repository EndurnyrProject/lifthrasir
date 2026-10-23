use crate::domain::world::map_scoped::MapScoped;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use lifthrasir_runtime::world::SceneOwner;
use lifthrasir_runtime::world::water::WaterSurface;

fn attach_client_water_scope(
    added: On<Add<WaterSurface>>,
    owners: Query<&SceneOwner>,
    scoped: Query<(), With<MapScoped>>,
    mut commands: Commands,
) {
    if let Ok(owner) = owners.get(added.entity)
        && scoped.contains(owner.0)
    {
        commands.entity(added.entity).insert(MapScoped);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_owned_water_keeps_client_map_cleanup_without_reparenting() {
        let mut app = App::new();
        app.add_observer(attach_client_water_scope);
        let owner = app.world_mut().spawn(MapScoped).id();
        let water = app
            .world_mut()
            .spawn((
                SceneOwner(owner),
                WaterSurface {
                    water_level: 1.0,
                    wave_height: 0.5,
                    wave_speed: 1.0,
                    wave_pitch: 20.0,
                    animation_speed: 1.0,
                    mesh_handle: Handle::default(),
                    material_handle: Handle::default(),
                },
            ))
            .id();
        app.world_mut().flush();
        assert!(app.world().get::<MapScoped>(water).is_some());
        assert!(app.world().get::<ChildOf>(water).is_none());
    }
}

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
        app.add_plugins(MapDomainPlugin)
            .add_observer(attach_client_water_scope);
    }
}
