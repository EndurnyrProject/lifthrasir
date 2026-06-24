use crate::infrastructure::ro_formats::StrFrame;
use bevy::{prelude::*, reflect::TypePath};

/// Resulting blend behaviour for an STR layer, mapped from the raw D3D
/// source/destination blend factor ints. These map onto Bevy `AlphaMode`s at
/// the render boundary (Task 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectBlend {
    /// Additive (`SrcAlpha` -> `One`).
    Add,
    /// Standard alpha blend (`SrcAlpha` -> `OneMinusSrcAlpha`).
    Blend,
    /// Multiplicative family.
    Multiply,
}

// D3D / wgpu blend factor ints as they appear in STR frame data, see
// `parse_blend_factor` in korangar's effect loader (repo-root reference.xml).
const BLEND_ONE: i32 = 2;
const BLEND_SRC_ALPHA: i32 = 5;
const BLEND_ONE_MINUS_SRC_ALPHA: i32 = 6;
const BLEND_ZERO: i32 = 1;
const BLEND_DST: i32 = 9;
const BLEND_SRC: i32 = 3;

/// Decode a raw STR `(src, dst)` blend factor pair into an `EffectBlend`.
///
/// - `SrcAlpha -> One` (the common additive case) -> `Add`.
/// - `SrcAlpha -> OneMinusSrcAlpha` (standard alpha) -> `Blend`.
/// - anything that multiplies source by a destination colour term -> `Multiply`.
///
/// Unknown / unhandled pairs default to `Blend`: it is the safest visible
/// fallback (effects are non-critical and degrade rather than panic, per D6).
pub fn decode_blend(src: i32, dst: i32) -> EffectBlend {
    match (src, dst) {
        (BLEND_SRC_ALPHA, BLEND_ONE) => EffectBlend::Add,
        (BLEND_SRC_ALPHA, BLEND_ONE_MINUS_SRC_ALPHA) => EffectBlend::Blend,
        // Multiply family: destination colour is scaled by the source (or the
        // source is zeroed and scaled by a destination colour term).
        (_, BLEND_SRC) | (_, BLEND_DST) | (BLEND_ZERO, _) | (BLEND_DST, _) => EffectBlend::Multiply,
        _ => EffectBlend::Blend,
    }
}

/// Per-frame data the renderer needs, distilled from `StrFrame`.
#[derive(Debug, Clone)]
pub struct LoadedFrame {
    pub frame_index: usize,
    pub xy: [f32; 8],
    pub uv: [f32; 8],
    pub texture_index: usize,
    pub color: [f32; 4],
    pub angle: f32,
    pub blend: EffectBlend,
}

/// One STR layer: its textures, the precomputed per-key frame index map, the
/// frames themselves and the layer-level blend (taken from its first frame).
#[derive(Debug, Clone)]
pub struct LoadedLayer {
    pub textures: Vec<Handle<Image>>,
    pub frame_index_map: Vec<Option<usize>>,
    pub frames: Vec<LoadedFrame>,
    pub blend: EffectBlend,
}

/// A fully prepared STR effect: parsed layers with resolved texture handles,
/// decoded blends and per-key frame index maps. Shared via `Handle` so repeated
/// casts of the same skill reuse the parsed data + textures.
#[derive(Asset, TypePath, Debug, Clone)]
pub struct LoadedEffectAsset {
    pub fps: u32,
    pub max_key: u32,
    pub layers: Vec<LoadedLayer>,
}

/// Expand a layer's sparse `frame_index`es into a dense `max_key`-length map,
/// where each key slot holds the index (into the layer `frames` slice) of the
/// frame active at that key, or `None` if no frame is active.
///
/// Ported verbatim from korangar's effect loader (reference.xml) so the runtime
/// frame interpolation (Task 6) stays consistent with the same index semantics.
pub fn build_frame_index_map(frames: &[StrFrame], max_key: usize) -> Vec<Option<usize>> {
    let mut map: Vec<Option<usize>> = Vec::with_capacity(max_key);

    if let Some(first) = frames.first() {
        let mut previous: Option<usize> = None;

        // Empty slots before the first frame becomes active.
        map.resize(first.frame_index.max(0) as usize, None);

        for (index, frame) in frames.iter().skip(1).enumerate() {
            let key = frame.frame_index.max(0) as usize;
            // Fill the gap up to this frame's key with the previously active frame.
            map.resize(map.len().max(key), previous);
            previous = Some(index);
        }

        // The last declared frame occupies its own key slot.
        map.push(previous);
    }

    // Pad the remaining keys with `None`.
    map.resize(max_key, None);

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_blend_additive_is_add() {
        // SrcAlpha (5) -> One (2)
        assert_eq!(decode_blend(5, 2), EffectBlend::Add);
    }

    #[test]
    fn decode_blend_normal_alpha_is_blend() {
        // SrcAlpha (5) -> OneMinusSrcAlpha (6)
        assert_eq!(decode_blend(5, 6), EffectBlend::Blend);
    }

    #[test]
    fn decode_blend_multiply_family_is_multiply() {
        // Zero (1) -> Src (3): destination tinted by the source colour.
        assert_eq!(decode_blend(1, 3), EffectBlend::Multiply);
    }

    #[test]
    fn decode_blend_unknown_defaults_to_blend() {
        assert_eq!(decode_blend(7, 8), EffectBlend::Blend);
    }

    fn frame_at(frame_index: i32) -> StrFrame {
        StrFrame {
            frame_index,
            frame_type: 0,
            offset: bevy::math::Vec2::ZERO,
            uv: [0.0; 8],
            xy: [0.0; 8],
            texture_index: 0.0,
            animation_type: 0,
            delay: 0.0,
            angle: 0.0,
            color: [0.0; 4],
            src_blend: 0,
            dst_blend: 0,
            mt_present: 0,
        }
    }

    #[test]
    fn build_frame_index_map_expands_sparse_frames_to_dense_map() {
        // Frames active at keys 0, 2 and 4; max_key 6.
        let frames = vec![frame_at(0), frame_at(2), frame_at(4)];
        let map = build_frame_index_map(&frames, 6);

        // Dense map is exactly max_key long.
        assert_eq!(map.len(), 6);
        // Keys before the second frame's index hold `previous` (None initially).
        assert_eq!(map[0], None);
        assert_eq!(map[1], None);
        // Keys 2..4 carry frame 0 (Some(0)).
        assert_eq!(map[2], Some(0));
        assert_eq!(map[3], Some(0));
        // The last declared frame fills its own key (Some(1)), then padding None.
        assert_eq!(map[4], Some(1));
        assert_eq!(map[5], None);
    }

    #[test]
    fn build_frame_index_map_empty_frames_is_all_none() {
        let map = build_frame_index_map(&[], 4);
        assert_eq!(map, vec![None, None, None, None]);
    }
}
