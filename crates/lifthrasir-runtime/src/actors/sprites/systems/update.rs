use crate::actors::sprites::components::RenderLayer;
use crate::rendering::SpriteRenderingSystems;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

/// Cleanup orphaned render layer children when parent despawns.
#[auto_add_system(
    plugin = crate::actors::ActorRenderPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::OrphanCleanup)
)]
pub fn cleanup_orphaned_sprites(
    mut commands: Commands,
    orphans: Query<Entity, (With<RenderLayer>, Without<ChildOf>)>,
) {
    for entity in orphans.iter() {
        commands.entity(entity).despawn();
    }
}
