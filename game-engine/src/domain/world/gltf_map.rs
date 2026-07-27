use bevy::prelude::*;

/// Runtime plugin for the `map-gltf` pipeline (unified per-map glb loading).
/// Scaffold only for now; the `LifExtensionHandler` and adapter systems are
/// wired in by later tasks in the glTF map pipeline.
pub struct GltfMapPlugin;

impl Plugin for GltfMapPlugin {
    fn build(&self, _app: &mut App) {}
}
