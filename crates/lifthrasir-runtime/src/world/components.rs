use bevy::prelude::*;
use lifthrasir_data::lif::{LifAudio, LifEffect, LifMap, LifProp, LifWater};
use ro_formats::RoAltitude;

/// Map-wide data decoded from the glb's root extensions, on the scene root.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(opaque)]
#[reflect(Component, Debug, Clone)]
pub struct LifMapData {
    pub altitude: RoAltitude,
    pub water: Option<LifWater>,
    pub water_tiles: Vec<(usize, usize)>,
    pub meta: LifMap,
}

/// On the scene root when the `LIF_*` payload could not be read. Mutually
/// exclusive with [`LifMapData`].
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(opaque)]
#[reflect(Component, Debug, Clone)]
pub struct LifMapLoadError(pub String);

/// A node carrying `lif_audio` extras; its transform is the emitter position.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(opaque)]
#[reflect(Component, Debug, Clone)]
pub struct LifAudioEmitter(pub LifAudio);

/// A node carrying `lif_effect` extras; its transform is the emitter position.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(opaque)]
#[reflect(Component, Debug, Clone)]
pub struct LifEffectEmitter(pub LifEffect);

/// A node carrying `lif_prop` extras; its transform is the baked RSW placement
/// of the referenced converted prop model.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(opaque)]
#[reflect(Component, Debug, Clone)]
pub struct LifPropRef(pub LifProp);

/// Original glTF node index for an entity spawned from a Lifthrasir map.
/// Editor tooling uses this stable identity to write node-transform edits back
/// into the same GLB document.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component, Debug, Clone)]
pub struct LifMapNodeIndex(pub usize);

/// Scene-derived shading input copied to each prop scene root.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct PropSceneContext {
    pub no_shade_tint: [f32; 3],
}

impl Default for PropSceneContext {
    fn default() -> Self {
        Self {
            no_shade_tint: [1.0; 3],
        }
    }
}

/// Caller-owned scene request; path resolution policy stays outside the decoder.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[require(Transform, Visibility, MapSceneStatus)]
pub struct MapSceneRequest {
    pub path: String,
}

/// Path-qualified strong scene handle; callers may seed a matching pair for reuse.
#[derive(Component, Debug, Clone)]
pub struct MapSceneHandle {
    pub path: String,
    pub scene: Handle<WorldAsset>,
}

/// Explicit ownership of a generated map-scene entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneOwner(pub Entity);

/// Persistent loading outcome for one caller-owned map root.
#[derive(Component, Default, Debug, Clone, PartialEq, Eq)]
pub enum MapSceneStatus {
    #[default]
    Loading,
    Ready,
    Failed(String),
}
