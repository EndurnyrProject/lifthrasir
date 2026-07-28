use lifthrasir_data::lif::LifUvAnimation;

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedModel {
    pub duration_ms: f32,
    pub textures: Vec<String>,
    pub roots: Vec<usize>,
    pub nodes: Vec<NormalizedNode>,
    pub materials: Vec<NormalizedMaterial>,
    pub provenance: ModelProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelProvenance {
    pub source_version: String,
    pub source_hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedNode {
    pub name: String,
    pub parent: Option<usize>,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub matrix: Option<[f32; 16]>,
    pub translation_track: NormalizedTrack<[f32; 3]>,
    pub rotation_track: NormalizedTrack<[f32; 4]>,
    pub scale_track: NormalizedTrack<[f32; 3]>,
    pub primitives: Vec<NormalizedPrimitive>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedTrack<T> {
    pub keys: Vec<NormalizedKey<T>>,
}

impl<T> Default for NormalizedTrack<T> {
    fn default() -> Self {
        Self { keys: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedKey<T> {
    pub time_ms: f32,
    pub value: T,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedPrimitive {
    pub material: usize,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uv0: Vec<[f32; 2]>,
    pub uv1: Option<Vec<[f32; 2]>>,
    pub indices: Vec<u32>,
    pub uv_animation: Option<LifUvAnimation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedMaterial {
    pub texture: Option<usize>,
    pub alpha: AlphaMode,
    pub two_sided: bool,
    pub shading: ShadingPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlphaMode {
    Mask { cutoff: f32 },
    Blend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadingPolicy {
    None,
    Flat,
    Smooth,
}
