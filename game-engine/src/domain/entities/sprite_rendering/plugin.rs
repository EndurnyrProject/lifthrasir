use crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin;
use bevy::prelude::*;

/// Wrapper plugin for sprite rendering.
///
/// All domain logic (resources, events, systems) is handled by SpriteRenderingDomainPlugin.
/// This wrapper exists for organizational purposes and maintains the public API.
pub struct GenericSpriteRenderingPlugin;

impl Plugin for GenericSpriteRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SpriteRenderingDomainPlugin);
        info!("GenericSpriteRenderingPlugin initialized");
    }
}
