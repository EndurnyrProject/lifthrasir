use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

/// Sprite and converted-model playback after renderer/content assets and RuntimeCorePlugin.
#[derive(AutoPlugin)]
pub struct ActorRenderPlugin;

impl Plugin for ActorRenderPlugin {
    #[auto_plugin]
    fn build(&self, app: &mut App) {
        crate::actors::models::register(app);
        if !app.world().contains_resource::<Assets<StandardMaterial>>() {
            app.init_asset::<StandardMaterial>();
        }
    }
}
