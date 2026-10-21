pub mod components;
pub mod emitters;
pub mod gltf;
pub use emitters::MapEmittersPlugin;
pub mod loading;
pub use components::MapSceneHandle;
pub use loading::{MapScenePlugin, MapSceneSystems};
pub mod props;
pub mod terrain;
pub mod water;
pub use components::PropSceneContext;
pub use components::{
    LifAudioEmitter, LifEffectEmitter, LifMapData, LifMapLoadError, LifMapNodeIndex, LifPropRef,
    MapSceneRequest, MapSceneStatus, SceneOwner,
};
pub use gltf::MapGltfPlugin;
pub use props::PropRenderPlugin;
pub use water::WaterRenderPlugin;

/// Reusable scene decoding, loading, prop/water rendering and owned emitter lifecycle.
/// Install after Bevy glTF/renderer infrastructure and shared content assets/catalogs.
pub struct WorldRenderPlugin;
impl bevy::prelude::Plugin for WorldRenderPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugins((
            MapGltfPlugin,
            MapScenePlugin,
            PropRenderPlugin,
            WaterRenderPlugin,
            MapEmittersPlugin,
        ));
    }
}
