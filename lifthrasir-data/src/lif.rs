//! Shared, bevy-free `LIF_*` glTF extension schemas for the unified map
//! pipeline. Depended on by the offline converter (`ro-to-lifthrasir-cli`)
//! and the runtime (`game-engine`); knows nothing about either.

use ro_formats::{GatError, RoAltitude};
use serde::{Deserialize, Serialize};

/// Format version written by the current converter and required by the
/// runtime handler.
pub const FORMAT_VERSION: u32 = 3;

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

pub const EXTRAS_UV_ANIMATION: &str = "lif_uv_animation";
pub const EXTRAS_NO_SHADE: &str = "lif_no_shade";

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
    #[serde(default = "white_tint")]
    pub no_shade_tint: [f32; 3],
    /// `true` for maps listed in `data/indoorrswtable.txt`. Drives the
    /// restricted indoor camera and the baked indoor lighting/exposure.
    #[serde(default)]
    pub indoor: bool,
    /// Baked `GlobalAmbientLight::brightness` (lux). Indoor and outdoor maps
    /// carry different floors; the runtime applies it verbatim.
    #[serde(default = "default_ambient_brightness")]
    pub ambient_brightness: f32,
    /// Baked camera `Exposure::ev100` for this map (outdoor = 15 / SUNLIGHT,
    /// indoor = 9.7 / BLENDER). The runtime pins the follow camera to it so
    /// the baked light values read correctly.
    #[serde(default = "default_exposure_ev100")]
    pub exposure_ev100: f32,
}

fn white_tint() -> [f32; 3] {
    [1.0; 3]
}

/// Legacy ambient floor (Bevy `lux::OFFICE`), used only when deserializing a
/// glb that predates the field.
fn default_ambient_brightness() -> f32 {
    320.0
}

/// Legacy camera exposure (`Exposure::BLENDER`, the Bevy default), used only
/// when deserializing a glb that predates the field.
fn default_exposure_ev100() -> f32 {
    9.7
}

pub fn no_shade_tint(ambient: [f32; 3], diffuse: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|index| {
        let low = ambient[index].min(diffuse[index]);
        let high = ambient[index].max(diffuse[index]);
        (high + (1.0 - high) * low).min(1.0)
    })
}

/// Root extension `LIF_model`: format identity and source-file provenance
/// for a converted prop glb.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifModel {
    pub format_version: u32,
    pub rsm_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifUvAnimation {
    pub duration_ms: u32,
    pub channels: Vec<LifUvChannel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifUvChannel {
    pub property: LifUvProperty,
    pub keys: Vec<LifScalarKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifUvProperty {
    TranslateU,
    TranslateV,
    ScaleU,
    ScaleV,
    Rotate,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LifScalarKey {
    pub time_ms: u32,
    pub value: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LifUvSample {
    pub translation: [f32; 2],
    pub scale: [f32; 2],
    pub rotation: f32,
}

impl Default for LifUvSample {
    fn default() -> Self {
        Self {
            translation: [0.0; 2],
            scale: [1.0; 2],
            rotation: 0.0,
        }
    }
}

impl LifUvSample {
    /// Row-major `T(center + translation) × Scale × Rotation × T(-center)`.
    pub fn matrix3(self) -> [f32; 9] {
        let (sin, cos) = self.rotation.sin_cos();
        let (sx, sy) = (self.scale[0], self.scale[1]);
        let (a, b, c, d) = (sx * cos, -sx * sin, sy * sin, sy * cos);
        let tx = 0.5 + self.translation[0] - 0.5 * (a + b);
        let ty = 0.5 + self.translation[1] - 0.5 * (c + d);
        [a, b, tx, c, d, ty, 0.0, 0.0, 1.0]
    }
}

impl LifUvAnimation {
    pub fn validate(&self) -> Result<(), String> {
        let mut properties = std::collections::HashSet::new();
        for channel in &self.channels {
            if !properties.insert(channel.property) {
                return Err(format!("duplicate UV channel: {:?}", channel.property));
            }
            let mut previous = None;
            for key in &channel.keys {
                if !key.value.is_finite() {
                    return Err(format!("non-finite UV key: {:?}", channel.property));
                }
                if key.time_ms > self.duration_ms
                    || previous.is_some_and(|time| key.time_ms <= time)
                {
                    return Err(format!("invalid UV key time: {:?}", channel.property));
                }
                previous = Some(key.time_ms);
            }
        }
        Ok(())
    }

    pub fn sample(&self, time_ms: u32, repeat: bool) -> Result<LifUvSample, String> {
        self.validate()?;
        let tick = if repeat {
            time_ms % self.duration_ms.max(1)
        } else {
            time_ms.min(self.duration_ms)
        };
        let mut sample = LifUvSample::default();
        for channel in &self.channels {
            if channel.keys.is_empty() {
                continue;
            }
            let value = sample_channel(channel, tick, self.duration_ms);
            match channel.property {
                LifUvProperty::TranslateU => sample.translation[0] += value,
                LifUvProperty::TranslateV => sample.translation[1] += value,
                LifUvProperty::ScaleU => sample.scale[0] = value,
                LifUvProperty::ScaleV => sample.scale[1] = value,
                LifUvProperty::Rotate => sample.rotation = value,
            }
        }
        Ok(sample)
    }
}

fn sample_channel(channel: &LifUvChannel, tick: u32, duration: u32) -> f32 {
    let next = channel.keys.partition_point(|key| tick >= key.time_ms);
    if next == channel.keys.len() {
        return channel.keys.last().expect("non-empty channel").value;
    }
    let (previous_time, previous_value) = if next == 0 {
        (0, 0.0)
    } else {
        let key = channel.keys[next - 1];
        (key.time_ms, key.value)
    };
    let next_key = channel.keys[next];
    let next_time = next_key.time_ms.min(duration);
    if next_time == previous_time {
        return next_key.value;
    }
    let factor = (tick - previous_time) as f32 / (next_time - previous_time) as f32;
    previous_value + factor * (next_key.value - previous_value)
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

/// Parameters for one cell of a [`LifWater`] zone grid.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LifWaterZone {
    pub level: f32,
    pub water_type: u32,
    pub wave_height: f32,
    pub wave_speed: f32,
    pub wave_pitch: f32,
    pub anim_speed: u32,
}

/// Root extension `LIF_water`: a row-major zone grid plus GND tile dimensions
/// and a bufferView index into the glb bin chunk. The bufferView carries a
/// row-major, LSB-first bitmask of selected water tiles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifWater {
    pub split_width: u32,
    pub split_height: u32,
    pub zones: Vec<LifWaterZone>,
    pub width: u32,
    pub height: u32,
    pub buffer_view: usize,
}

impl LifWater {
    pub fn zone_at(&self, x: usize, y: usize) -> &LifWaterZone {
        &self.zones[self.zone_index_at(x, y)]
    }

    pub fn zone_index_at(&self, x: usize, y: usize) -> usize {
        let zone_x = zone_index(x, self.width, self.split_width);
        let zone_y = zone_index(y, self.height, self.split_height);
        zone_y * self.split_width as usize + zone_x
    }
}

fn zone_index(cell: usize, cells: u32, splits: u32) -> usize {
    if cells == 0 || splits == 0 {
        return 0;
    }
    ((cell * splits as usize) / cells as usize).min(splits as usize - 1)
}

pub fn encode_water_mask(tiles: &[(usize, usize)], width: usize, height: usize) -> Vec<u8> {
    let cell_count = width
        .checked_mul(height)
        .expect("water mask dimensions overflow");
    let mut bytes = vec![0; cell_count.div_ceil(8)];
    for &(x, y) in tiles {
        assert!(x < width && y < height, "water tile is outside the mask");
        let index = y * width + x;
        bytes[index / 8] |= 1 << (index % 8);
    }
    bytes
}

pub fn decode_water_mask(bytes: &[u8], width: usize, height: usize) -> Vec<(usize, usize)> {
    let cell_count = width
        .checked_mul(height)
        .expect("water mask dimensions overflow");
    assert_eq!(
        bytes.len(),
        cell_count.div_ceil(8),
        "water mask length does not match its dimensions"
    );
    (0..cell_count)
        .filter(|&index| bytes[index / 8] & (1 << (index % 8)) != 0)
        .map(|index| (index % width, index / width))
        .collect()
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
            no_shade_tint: [0.5, 0.6, 0.7],
            indoor: true,
            ambient_brightness: 100.0,
            exposure_ev100: 9.7,
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
            split_width: 2,
            split_height: 1,
            zones: vec![
                LifWaterZone {
                    level: 5.0,
                    water_type: 2,
                    wave_height: 0.5,
                    wave_speed: 1.5,
                    wave_pitch: 40.0,
                    anim_speed: 4,
                },
                LifWaterZone {
                    level: 8.0,
                    water_type: 3,
                    wave_height: 0.25,
                    wave_speed: 2.0,
                    wave_pitch: 30.0,
                    anim_speed: 6,
                },
            ],
            width: 100,
            height: 80,
            buffer_view: 3,
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: LifWater = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original, deserialized);
        assert_eq!(deserialized.zone_at(0, 0), &deserialized.zones[0]);
        assert_eq!(deserialized.zone_at(99, 79), &deserialized.zones[1]);
    }

    #[test]
    fn water_mask_round_trips_in_row_major_order() {
        let tiles = [(0, 0), (2, 1), (3, 1), (4, 2)];
        let bytes = encode_water_mask(&tiles, 5, 3);

        assert_eq!(bytes, [0b1000_0001, 0b0100_0001]);
        assert_eq!(decode_water_mask(&bytes, 5, 3), tiles);

        let unordered = [(4, 2), (3, 1), (2, 1), (0, 0)];
        assert_eq!(
            decode_water_mask(&encode_water_mask(&unordered, 5, 3), 5, 3),
            tiles
        );
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
    fn uv_animation_samples_browedit_defaults_interpolation_and_wrap() {
        let animation = LifUvAnimation {
            duration_ms: 1_000,
            channels: vec![
                LifUvChannel {
                    property: LifUvProperty::TranslateU,
                    keys: vec![
                        LifScalarKey {
                            time_ms: 200,
                            value: 1.0,
                        },
                        LifScalarKey {
                            time_ms: 600,
                            value: 3.0,
                        },
                    ],
                },
                LifUvChannel {
                    property: LifUvProperty::ScaleU,
                    keys: vec![LifScalarKey {
                        time_ms: 200,
                        value: 2.0,
                    }],
                },
            ],
        };

        let before = animation.sample(100, false).unwrap();
        let middle = animation.sample(400, false).unwrap();
        let wrapped = animation.sample(1_400, true).unwrap();

        assert_eq!(before.translation[0], 0.5);
        assert_eq!(before.scale[0], 1.0);
        assert_eq!(middle.translation[0], 2.0);
        assert_eq!(wrapped, middle);
        assert_eq!(middle.scale[1], 1.0);
    }

    #[test]
    fn uv_schema_round_trips_and_composes_around_texture_center() {
        let animation = LifUvAnimation {
            duration_ms: 1_000,
            channels: vec![LifUvChannel {
                property: LifUvProperty::Rotate,
                keys: vec![LifScalarKey {
                    time_ms: 0,
                    value: 1.0,
                }],
            }],
        };
        let json = serde_json::to_string(&animation).unwrap();
        let decoded: LifUvAnimation = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, animation);
        assert_eq!(EXTRAS_UV_ANIMATION, "lif_uv_animation");
        assert_eq!(EXTRAS_NO_SHADE, "lif_no_shade");

        let matrix = LifUvSample {
            translation: [0.1, -0.2],
            scale: [2.0, 3.0],
            rotation: std::f32::consts::FRAC_PI_2,
        }
        .matrix3();
        let transformed = [
            matrix[0] + matrix[1] * 0.5 + matrix[2],
            matrix[3] + matrix[4] * 0.5 + matrix[5],
        ];
        assert!((transformed[0] - 0.6).abs() < 1e-6);
        assert!((transformed[1] - 1.8).abs() < 1e-6);
    }

    #[test]
    fn uv_animation_validates_and_holds_terminal_values() {
        let animation = LifUvAnimation {
            duration_ms: 0,
            channels: vec![LifUvChannel {
                property: LifUvProperty::Rotate,
                keys: vec![LifScalarKey {
                    time_ms: 0,
                    value: 2.0,
                }],
            }],
        };
        assert_eq!(animation.sample(500, true).unwrap().rotation, 2.0);
        assert_eq!(animation.sample(500, false).unwrap().rotation, 2.0);

        let mut invalid = animation.clone();
        invalid.channels[0].keys[0].value = f32::NAN;
        assert!(invalid.validate().is_err());
        invalid.channels[0].keys[0].value = 1.0;
        invalid.channels.push(invalid.channels[0].clone());
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn no_shade_tint_matches_formula_and_current_maps_default_white() {
        let tint = no_shade_tint([0.2, 0.4, 0.8], [0.6, 0.3, 0.5]);
        for (actual, expected) in tint.into_iter().zip([0.68, 0.58, 0.9]) {
            assert!((actual - expected).abs() < 1e-6);
        }
        let current = r#"{"format_version":2,"rsw_hash":"a","gnd_hash":"b","gat_hash":"c","ambient_color":[0.3,0.3,0.4]}"#;
        let map: LifMap = serde_json::from_str(current).unwrap();
        assert_eq!(map.no_shade_tint, [1.0; 3]);
        assert!(!map.indoor);
        assert_eq!(map.ambient_brightness, 320.0);
        assert_eq!(map.exposure_ev100, 9.7);
    }

    #[test]
    fn lif_prop_defaults_anim_fields_for_older_glbs() {
        let json = r#"{"model":"ro://data/model/prontera/tree.rsm"}"#;

        let deserialized: LifProp = serde_json::from_str(json).expect("deserialize");

        assert_eq!(deserialized.anim_type, 0);
        assert_eq!(deserialized.anim_speed, 1.0);
    }
}
