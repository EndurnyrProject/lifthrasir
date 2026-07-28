//! Shared, bevy-free `LIF_*` glTF extension schemas for the unified map
//! pipeline. Depended on by the offline converter (`ro-to-lifthrasir-cli`)
//! and the runtime (`game-engine`); knows nothing about either.

use ro_formats::{GatError, RoAltitude, RswWater};
use serde::{Deserialize, Serialize};

/// Format version written by the current converter and required by the
/// runtime handler.
pub const FORMAT_VERSION: u32 = 1;

/// glTF root-extension key for [`LifMap`].
pub const EXTENSION_MAP: &str = "LIF_map";

/// glTF root-extension key for [`LifWater`].
pub const EXTENSION_WATER: &str = "LIF_water";

/// glTF root-extension key for [`LifGat`].
pub const EXTENSION_GAT: &str = "LIF_gat";

/// glTF root-extension key for [`LifModel`].
pub const EXTENSION_MODEL: &str = "LIF_model";

/// Node-extras key for [`LifAudio`].
pub const EXTRAS_AUDIO: &str = "lif_audio";

/// Node-extras key for [`LifEffect`].
pub const EXTRAS_EFFECT: &str = "lif_effect";

/// Node-extras key for [`LifProp`].
pub const EXTRAS_PROP: &str = "lif_prop";

/// Root extension `LIF_map`: format identity, source-file provenance, and the
/// ambient light that has no `KHR_lights_punctual` equivalent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifMap {
    pub format_version: u32,
    pub rsw_hash: String,
    pub gnd_hash: String,
    pub gat_hash: String,
    /// Mirrors `RswLight::ambient` (RGB, no alpha).
    pub ambient_color: [f32; 3],
}

/// Root extension `LIF_model`: format identity and source-file provenance
/// for a converted prop glb.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifModel {
    pub format_version: u32,
    pub rsm_hash: String,
}

/// Root extension `LIF_gat`: GAT dims plus the bufferView index into the glb
/// bin chunk. The bufferView carries the original `.gat` file bytes verbatim
/// (no re-encoding); `width`/`height` here are convenience/validation
/// metadata against the parsed content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifGat {
    pub width: u32,
    pub height: u32,
    pub buffer_view: u32,
}

impl LifGat {
    /// Parses the bufferView's raw `.gat` bytes. A thin delegation to
    /// `ro_formats::RoAltitude::from_bytes` -- the bin chunk holds the
    /// original file content, so there is nothing to reverse ourselves.
    pub fn decode(bytes: &[u8]) -> Result<RoAltitude, GatError> {
        RoAltitude::from_bytes(bytes)
    }

    /// Confirms `raw` parses to the dims this extension declares. Byte
    /// identity against the source `.gat` is the converter's job (a memcmp);
    /// this only checks the two are talking about the same grid.
    pub fn validate(&self, raw: &[u8]) -> Result<(), GatError> {
        let altitude = RoAltitude::from_bytes(raw)?;
        if altitude.width != self.width || altitude.height != self.height {
            return Err(GatError::ParseError(format!(
                "LIF_gat dims mismatch: extension declares {}x{}, parsed bytes are {}x{}",
                self.width, self.height, altitude.width, altitude.height
            )));
        }
        Ok(())
    }
}

/// Root extension `LIF_water`: mirrors `RswWater` verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LifWater {
    pub level: f32,
    pub water_type: u32,
    pub wave_height: f32,
    pub wave_speed: f32,
    pub wave_pitch: f32,
    pub anim_speed: u32,
}

impl From<&RswWater> for LifWater {
    fn from(water: &RswWater) -> Self {
        Self {
            level: water.level,
            water_type: water.water_type,
            wave_height: water.wave_height,
            wave_speed: water.wave_speed,
            wave_pitch: water.wave_pitch,
            anim_speed: water.anim_speed,
        }
    }
}

impl From<LifWater> for RswWater {
    fn from(water: LifWater) -> Self {
        Self {
            level: water.level,
            water_type: water.water_type,
            wave_height: water.wave_height,
            wave_speed: water.wave_speed,
            wave_pitch: water.wave_pitch,
            anim_speed: water.anim_speed,
        }
    }
}

/// Node extras: audio emitter params mirroring `RswSound` (position lives in
/// the node transform, not here).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifAudio {
    pub name: String,
    pub file: String,
    pub volume: f32,
    pub range: f32,
    pub cycle: f32,
}

/// Node extras: effect emitter params mirroring `RswEffect` (position lives
/// in the node transform, not here).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifEffect {
    pub name: String,
    pub effect_type: u32,
    pub emit_speed: f32,
    pub params: [f32; 4],
}

/// Node extras: a prop reference to a native RSM model, mirroring
/// `RswModel::anim_type`/`anim_speed` for props that animate in place (e.g.
/// windmills). `#[serde(default)]` keeps older glbs (no animation fields)
/// deserializing as static props, same as today.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifProp {
    pub model: String,
    #[serde(default)]
    pub anim_type: u32,
    #[serde(default = "default_anim_speed")]
    pub anim_speed: f32,
}

/// `RswModel::anim_speed`'s neutral value -- normal playback speed.
fn default_anim_speed() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a synthetic raw `.gat` file buffer matching the real format
    /// (`GRAT` + major/minor + width/height + per-cell `[f32; 4]` heights and
    /// a `u32` raw type), so tests exercise the same bytes the converter will
    /// write into the bufferView.
    fn build_raw_gat(width: u32, height: u32, raw_cell_type: u32) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(b"GRAT");
        buffer.push(2);
        buffer.push(2);
        buffer.extend_from_slice(&width.to_le_bytes());
        buffer.extend_from_slice(&height.to_le_bytes());

        for i in 0..(width * height) {
            let base = i as f32;
            for offset in 0..4 {
                buffer.extend_from_slice(&(base + offset as f32).to_le_bytes());
            }
            buffer.extend_from_slice(&raw_cell_type.to_le_bytes());
        }

        buffer
    }

    #[test]
    fn decode_matches_ro_altitude_from_bytes() {
        let raw = build_raw_gat(3, 2, 3);

        let decoded = LifGat::decode(&raw).expect("decode");
        let parsed = RoAltitude::from_bytes(&raw).expect("parse");

        assert_eq!(decoded.version, parsed.version);
        assert_eq!(decoded.width, parsed.width);
        assert_eq!(decoded.height, parsed.height);
        assert_eq!(decoded.cells.len(), parsed.cells.len());
        for (a, b) in decoded.cells.iter().zip(parsed.cells.iter()) {
            assert_eq!(a.height, b.height);
            assert_eq!(a.cell_type, b.cell_type);
        }
    }

    #[test]
    fn validate_passes_when_dims_match() {
        let raw = build_raw_gat(4, 5, 0);
        let extension = LifGat {
            width: 4,
            height: 5,
            buffer_view: 0,
        };

        assert!(extension.validate(&raw).is_ok());
    }

    #[test]
    fn validate_fails_when_dims_mismatch() {
        let raw = build_raw_gat(4, 5, 0);
        let extension = LifGat {
            width: 4,
            height: 6,
            buffer_view: 0,
        };

        assert!(extension.validate(&raw).is_err());
    }

    #[test]
    fn lif_map_serde_round_trip() {
        let original = LifMap {
            format_version: 1,
            rsw_hash: "abc123".to_string(),
            gnd_hash: "def456".to_string(),
            gat_hash: "ghi789".to_string(),
            ambient_color: [0.3, 0.3, 0.4],
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: LifMap = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn lif_model_serde_round_trip() {
        let original = LifModel {
            format_version: 1,
            rsm_hash: "abc123".to_string(),
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: LifModel = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn lif_gat_extension_serde_round_trip() {
        let original = LifGat {
            width: 200,
            height: 200,
            buffer_view: 3,
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: LifGat = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn lif_water_serde_round_trip() {
        let original = LifWater {
            level: 5.0,
            water_type: 2,
            wave_height: 0.5,
            wave_speed: 1.5,
            wave_pitch: 40.0,
            anim_speed: 4,
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: LifWater = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn lif_water_converts_from_and_to_rsw_water() {
        let rsw_water = RswWater {
            level: 5.0,
            water_type: 2,
            wave_height: 0.5,
            wave_speed: 1.5,
            wave_pitch: 40.0,
            anim_speed: 4,
        };

        let lif_water = LifWater::from(&rsw_water);
        let round_tripped = RswWater::from(lif_water);

        assert_eq!(round_tripped.level, rsw_water.level);
        assert_eq!(round_tripped.water_type, rsw_water.water_type);
        assert_eq!(round_tripped.wave_height, rsw_water.wave_height);
        assert_eq!(round_tripped.wave_speed, rsw_water.wave_speed);
        assert_eq!(round_tripped.wave_pitch, rsw_water.wave_pitch);
        assert_eq!(round_tripped.anim_speed, rsw_water.anim_speed);
    }

    #[test]
    fn lif_audio_serde_round_trip() {
        let original = LifAudio {
            name: "torch".to_string(),
            file: "effect/torch_wav.wav".to_string(),
            volume: 0.8,
            range: 10.0,
            cycle: 4.0,
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: LifAudio = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn lif_effect_serde_round_trip() {
        let original = LifEffect {
            name: "waterfall".to_string(),
            effect_type: 12,
            emit_speed: 0.25,
            params: [1.0, 2.0, 3.0, 4.0],
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: LifEffect = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn lif_prop_serde_round_trip() {
        let original = LifProp {
            model: "ro://data/model/prontera/tree.rsm".to_string(),
            anim_type: 1,
            anim_speed: 2.0,
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: LifProp = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn lif_prop_defaults_anim_fields_for_older_glbs() {
        let json = r#"{"model":"ro://data/model/prontera/tree.rsm"}"#;

        let deserialized: LifProp = serde_json::from_str(json).expect("deserialize");

        assert_eq!(deserialized.anim_type, 0);
        assert_eq!(deserialized.anim_speed, 1.0);
    }
}
