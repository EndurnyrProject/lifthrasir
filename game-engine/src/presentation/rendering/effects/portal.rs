#[cfg(test)]
use super::VfxSystems;
use crate::domain::entities::markers::WarpPortal;
use crate::utils::constants::CELL_SIZE;
use bevy::prelude::*;

pub use lifthrasir_runtime::effects::procedural::portal::*;

/// Bridge: a warp-portal NPC (domain `WarpPortal`) gets a `PortalVfx` so the
/// shared attach path below builds its surface + sparkles.
pub(super) fn attach_warp_portal_vfx(
    mut commands: Commands,
    warps: Query<Entity, Added<WarpPortal>>,
) {
    for entity in &warps {
        commands.entity(entity).insert(PortalVfx {
            color: Color::srgb(0.3, 0.8, 1.0),
            radius: CELL_SIZE,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warp_bridge_dresses_the_shared_portal_in_the_same_update() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .add_plugins(PortalVfxPlugin)
            .add_systems(Update, attach_warp_portal_vfx.before(VfxSystems));
        let portal = app
            .world_mut()
            .spawn((WarpPortal, Transform::default()))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<PortalVfx>(portal).unwrap().radius,
            CELL_SIZE
        );
        assert!(
            app.world()
                .get::<MeshMaterial3d<PortalMaterial>>(portal)
                .is_some()
        );
    }
}
