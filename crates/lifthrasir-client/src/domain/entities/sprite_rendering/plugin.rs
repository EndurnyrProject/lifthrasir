use bevy::prelude::*;

/// Wrapper plugin for sprite rendering.
///
/// All domain logic (resources, events, systems) is handled by SpriteRenderingDomainPlugin.
/// This wrapper exists for organizational purposes and maintains the public API.
///
/// Requires RuntimeCorePlugin for the shared billboard resources and phase ordering.
pub struct GenericSpriteRenderingPlugin;

impl Plugin for GenericSpriteRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SpriteRenderingDomainPlugin);
        debug!("GenericSpriteRenderingPlugin initialized");
    }
}

/// Auto-plugin collecting sprite rendering resources, events, and systems;
/// they register themselves via `auto_*` attributes in
/// `domain/entities/sprite_rendering/*`.
#[derive(bevy_auto_plugin::prelude::AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct SpriteRenderingDomainPlugin;
