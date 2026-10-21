//! Neutral, host-agnostic description of one map.
//!
//! The retail converter builds a [`MapDocument`] from RSW/GND/GAT bytes; the
//! world editor will build one from its own state. The map glb writer only
//! consumes this document, so neither host can grow a private glTF path.

use crate::converters::gltf_out::to_forward_slashes;
use crate::converters::map::terrain::TerrainPrimitive;
use crate::converters::map::textures::TextureOut;
use crate::converters::map::water;
use crate::report::{Diagnostic, DiagnosticLevel};
use anyhow::Context;
use glam::{Mat3, Quat, Vec3};
use lifthrasir_data::lif;
use ro_formats::{
    RoAltitude, RoGround, RoWorld, RswEffect, RswLight, RswLightObj, RswModel, RswObject, RswSound,
};
use std::collections::HashSet;

/// Historical pre-GLB lighting constants, originally mirrored from
/// `game-engine/src/presentation/rendering/lighting.rs`.
pub const SUN_MAX_LUX: f32 = 10_000.0;
pub const POINT_LIGHT_LUX_AT_HALF_RANGE: f32 = 500.0;

/// Indoor lighting/exposure, live-tuned against `prt_cas` at EV100 9.7 (see
/// `notes/map-lighting-tuning.md`). Point lights get an 18x intensity boost
/// (`500 -> 9000` lux-at-half-range) and doubled range; the RSW sun is pinned
/// to a dim interior value; ambient sits at a slightly higher floor.
pub const INDOOR_POINT_LIGHT_LUX_AT_HALF_RANGE: f32 = 9_000.0;
pub const INDOOR_POINT_LIGHT_RANGE_SCALE: f32 = 2.0;
pub const INDOOR_SUN_LUX: f32 = 200.0;
pub const INDOOR_AMBIENT_BRIGHTNESS: f32 = 100.0;
/// Indoor maps stay at Bevy's default `Exposure::BLENDER` (9.7), the exposure
/// the indoor light values were tuned against.
pub const INDOOR_EXPOSURE_EV100: f32 = 9.7;

/// Outdoor lighting/exposure. Outdoor maps are idiomatic daylight read at
/// EV100 10.5. The camera multiplier is `2^-ev100 / 1.2`; EV100 15 (the
/// original sunny-16 anchor) left even a 30k lux sun below middle grey, and
/// EV100 12 still read dim for RO's near-full-bright look. At EV100 10.5 the
/// baked sun (up to `SUN_MAX_LUX`) renders sunlit ground near white while the
/// ambient floor keeps shadowed terrain readable.
pub const OUTDOOR_AMBIENT_BRIGHTNESS: f32 = 2_500.0;
pub const OUTDOOR_EXPOSURE_EV100: f32 = 10.5;

/// A ground texture the document references, by the name its terrain
/// primitives use and the URI the glb stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureRef {
    pub name: String,
    pub uri: String,
}

/// A placed payload with its local transform. `translation` and `rotation` are
/// in the native RO frame; the writer applies the glTF root fix.
#[derive(Debug, Clone, PartialEq)]
pub struct MapNode<T> {
    pub name: String,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub payload: T,
}

/// One `KHR_lights_punctual` light. Retail RSW only produces [`MapLight::Sun`]
/// and [`MapLight::Point`]; the editor additionally authors spots.
#[derive(Debug, Clone, PartialEq)]
pub enum MapLight {
    Sun {
        rotation: Quat,
        color: [f32; 3],
        lux: f32,
    },
    Point {
        translation: Vec3,
        color: [f32; 3],
        candela: f32,
        range: f32,
    },
    /// Authored by the world editor; retail RSW has no spot lights.
    #[allow(dead_code)]
    Spot {
        translation: Vec3,
        rotation: Quat,
        color: [f32; 3],
        candela: f32,
        range: f32,
        inner_angle: f32,
        outer_angle: f32,
    },
}

/// Water zones plus the GND cells the converter selected for them.
#[derive(Debug, Clone, PartialEq)]
pub struct MapWater {
    pub zones: lif::LifWater,
    pub tiles: Vec<(usize, usize)>,
}

/// Everything the map glb writer needs, whether it came from retail files or
/// from the world editor.
#[derive(Debug, Clone)]
pub struct MapDocument {
    pub name: String,
    pub meta: lif::LifMap,
    pub textures: Vec<TextureRef>,
    pub terrain: Vec<TerrainPrimitive>,
    pub lights: Vec<MapLight>,
    pub props: Vec<MapNode<lif::LifProp>>,
    pub sounds: Vec<MapNode<lif::LifAudio>>,
    pub effects: Vec<MapNode<lif::LifEffect>>,
    pub water: Option<MapWater>,
    pub gat: RoAltitude,
}

/// The retail inputs [`MapDocument::from_retail`] reads.
pub struct RetailInputs<'a> {
    pub map_name: &'a str,
    pub ground: &'a RoGround,
    pub world: &'a RoWorld,
    pub primitives: &'a [TerrainPrimitive],
    pub textures: &'a [TextureOut],
    pub gat_bytes: &'a [u8],
    pub gnd_bytes: &'a [u8],
    pub rsw_bytes: &'a [u8],
    pub converted_models: &'a HashSet<String>,
    pub indoor: bool,
}

impl MapDocument {
    /// Builds a document from retail RSW/GND/GAT. Never edits the inputs; the
    /// returned diagnostics name every object dropped for carrying a number
    /// glTF cannot express.
    pub fn from_retail(inputs: RetailInputs<'_>) -> anyhow::Result<(Self, Vec<Diagnostic>)> {
        let world = inputs.world;
        let ground = inputs.ground;
        let map_width = ground.width as f32;
        let map_height = ground.height as f32;

        let mut diagnostics = Vec::new();
        for object in world
            .objects
            .iter()
            .filter(|object| !is_representable(object))
        {
            diagnostics.push(Diagnostic {
                level: DiagnosticLevel::Warning,
                message: format!(
                    "  not a finite placement, dropping {}: '{}'",
                    object_kind(object),
                    object_name(object)
                ),
            });
        }

        let mut lights = vec![MapLight::Sun {
            rotation: look_to_rotation(sun_direction(&world.light), Vec3::NEG_Y),
            color: world.light.diffuse,
            lux: sun_illuminance(&world.light, inputs.indoor),
        }];
        let mut props = Vec::new();
        let mut sounds = Vec::new();
        let mut effects = Vec::new();

        for object in emitted_objects(world) {
            match object {
                RswObject::Light(light) => lights.push(MapLight::Point {
                    translation: rsw_position_to_world(light.position, map_width, map_height),
                    color: light.color,
                    candela: point_light_candela(light, inputs.indoor),
                    range: point_light_range(light, inputs.indoor),
                }),
                RswObject::Sound(sound) => sounds.push(MapNode {
                    name: sound.name.clone(),
                    translation: rsw_position_to_world(sound.position, map_width, map_height),
                    rotation: Quat::IDENTITY,
                    scale: Vec3::ONE,
                    payload: lif_audio(sound),
                }),
                RswObject::Effect(effect) => effects.push(MapNode {
                    name: effect.name.clone(),
                    translation: rsw_position_to_world(effect.position, map_width, map_height),
                    rotation: Quat::IDENTITY,
                    scale: Vec3::ONE,
                    payload: lif_effect(effect),
                }),
                RswObject::Model(model) => props.push(MapNode {
                    name: model.name.clone(),
                    translation: rsw_position_to_world(model.position, map_width, map_height),
                    rotation: rsw_model_rotation(model),
                    scale: Vec3::from(model.scale),
                    payload: lif_prop(model, inputs.converted_models).with_context(|| {
                        format!("map '{}', prop '{}'", inputs.map_name, model.filename)
                    })?,
                }),
            }
        }

        let mut zones = water::resolve_water(ground, &world.water)?;
        let tiles = water::select_water_tiles(ground, &zones);
        let water = zones.zones.iter().any(|zone| zone.level != 0.0).then(|| {
            zones.buffer_view = 0;
            MapWater {
                zones: zones.clone(),
                tiles,
            }
        });

        let meta = lif::LifMap {
            format_version: lif::MAP_FORMAT_VERSION,
            source: Some(lif::LifSource {
                rsw_hash: crate::converters::gltf_out::hash_hex(inputs.rsw_bytes),
                gnd_hash: crate::converters::gltf_out::hash_hex(inputs.gnd_bytes),
                gat_hash: crate::converters::gltf_out::hash_hex(inputs.gat_bytes),
            }),
            ambient_color: world.light.ambient,
            no_shade_tint: lif::no_shade_tint(world.light.ambient, world.light.diffuse),
            indoor: inputs.indoor,
            ambient_brightness: if inputs.indoor {
                INDOOR_AMBIENT_BRIGHTNESS
            } else {
                OUTDOOR_AMBIENT_BRIGHTNESS
            },
            exposure_ev100: if inputs.indoor {
                INDOOR_EXPOSURE_EV100
            } else {
                OUTDOOR_EXPOSURE_EV100
            },
        };

        let document = Self {
            name: inputs.map_name.to_string(),
            meta,
            textures: inputs
                .textures
                .iter()
                .map(|texture| TextureRef {
                    name: texture.source_name.clone(),
                    uri: texture.relative_path.clone(),
                })
                .collect(),
            terrain: inputs.primitives.to_vec(),
            lights,
            props,
            sounds,
            effects,
            water,
            gat: RoAltitude::from_bytes(inputs.gat_bytes).map_err(|error| {
                anyhow::anyhow!("parsing GAT of map '{}': {error}", inputs.map_name)
            })?,
        };

        Ok((document, diagnostics))
    }

    /// Structural checks shared by every producer, independent of where the
    /// document came from. Returns every problem rather than the first.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if !is_valid_map_name(&self.name) {
            errors.push(format!(
                "map name '{}' must be a lowercase asset path segment (a-z, 0-9, _, -, @)",
                self.name
            ));
        }
        if self.meta.format_version != lif::MAP_FORMAT_VERSION {
            errors.push(format!(
                "LIF_map format_version {} is not {}",
                self.meta.format_version,
                lif::MAP_FORMAT_VERSION
            ));
        }
        if self.gat.cells.len() != (self.gat.width as usize) * (self.gat.height as usize) {
            errors.push(format!(
                "GAT declares {}x{} but carries {} cells",
                self.gat.width,
                self.gat.height,
                self.gat.cells.len()
            ));
        }
        for primitive in &self.terrain {
            if !self
                .textures
                .iter()
                .any(|texture| texture.name == primitive.texture)
            {
                errors.push(format!(
                    "terrain primitive references texture '{}' with no TextureRef",
                    primitive.texture
                ));
            }
        }
        for light in &self.lights {
            errors.extend(validate_light(light));
        }
        for texture in &self.textures {
            if !texture
                .uri
                .starts_with(crate::converters::map::textures::GROUND_TEXTURE_URI_PREFIX)
            {
                errors.push(format!(
                    "texture '{}' URI '{}' must start with {}",
                    texture.name,
                    texture.uri,
                    crate::converters::map::textures::GROUND_TEXTURE_URI_PREFIX
                ));
            }
            if !texture.uri.ends_with(".ktx2") {
                errors.push(format!(
                    "texture '{}' URI '{}' must be a .ktx2",
                    texture.name, texture.uri
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn validate_light(light: &MapLight) -> Vec<String> {
    let mut errors = Vec::new();
    match light {
        MapLight::Sun {
            rotation,
            color,
            lux,
        } => {
            if !is_unit(*rotation) {
                errors.push("sun rotation is not a unit quaternion".to_string());
            }
            if !finite_all(color.iter().copied().chain([*lux])) {
                errors.push("sun color or lux is not finite".to_string());
            }
        }
        MapLight::Spot {
            translation,
            rotation,
            color,
            candela,
            range,
            inner_angle,
            outer_angle,
        } => {
            if !finite_all(
                translation
                    .to_array()
                    .into_iter()
                    .chain(color.iter().copied())
                    .chain([*candela, *range, *inner_angle, *outer_angle]),
            ) {
                errors.push("spot light transform or intensity is not finite".to_string());
            }
            if !is_unit(*rotation) {
                errors.push("spot light rotation is not a unit quaternion".to_string());
            }
            if !(0.0 < *inner_angle && *inner_angle <= *outer_angle) {
                errors.push(format!(
                    "spot light inner angle {inner_angle} must be > 0 and <= outer angle {outer_angle}"
                ));
            }
            if *outer_angle > std::f32::consts::FRAC_PI_2 {
                errors.push(format!(
                    "spot light outer angle {outer_angle} must be <= PI/2"
                ));
            }
        }
        MapLight::Point {
            translation,
            color,
            candela,
            range,
        } => {
            if !finite_all(
                translation
                    .to_array()
                    .into_iter()
                    .chain(color.iter().copied())
                    .chain([*candela, *range]),
            ) {
                errors.push("point light transform or intensity is not finite".to_string());
            }
        }
    }
    errors
}

fn finite_all(values: impl IntoIterator<Item = f32>) -> bool {
    values.into_iter().all(f32::is_finite)
}

fn is_unit(rotation: Quat) -> bool {
    (rotation.length_squared() - 1.0).abs() < 1e-3
}

/// A map name is one asset path segment, so it must be non-empty and free of
/// separators, spaces and uppercase. Everything else retail already uses is
/// allowed: `a-z`, `0-9`, `_`, `-` and `@` cover `1@pump`, `pvp_y_1-2` and
/// every other shipped map.
pub fn is_valid_map_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '@'))
}

fn object_kind(object: &RswObject) -> &'static str {
    match object {
        RswObject::Model(_) => "prop",
        RswObject::Light(_) => "light",
        RswObject::Sound(_) => "sound",
        RswObject::Effect(_) => "effect",
    }
}

/// Preserves the historical `game-engine/src/utils/coordinates.rs::rsw_position_to_bevy` math.
pub fn rsw_position_to_world(position: [f32; 3], map_width: f32, map_height: f32) -> Vec3 {
    Vec3::new(
        position[0] + (map_width * 5.0),
        position[1],
        position[2] + (map_height * 5.0),
    )
}

/// Preserves historical `game-engine/src/utils/coordinates.rs::rsw_to_bevy_transform` math
/// (mat4.rotateZ, then rotateX, then rotateY).
pub fn rsw_model_rotation(model: &RswModel) -> Quat {
    Quat::from_rotation_z(model.rotation[2].to_radians())
        * Quat::from_rotation_x(model.rotation[0].to_radians())
        * Quat::from_rotation_y(model.rotation[1].to_radians())
}

/// Port of Bevy's `Transform::look_to`, used by the native directional light.
pub(crate) fn look_to_rotation(direction: Vec3, up: Vec3) -> Quat {
    let back = -direction;
    let right = up
        .cross(back)
        .try_normalize()
        .unwrap_or_else(|| up.any_orthonormal_vector());
    let up = back.cross(right);
    Quat::from_mat3(&Mat3::from_cols(right, up, back))
}

/// World-space direction the RSW sun shines along, mirroring
/// `lighting.rs::setup_directional_light`.
pub fn sun_direction(light: &RswLight) -> Vec3 {
    let elevation = (90.0 - light.latitude as f32).to_radians();
    let azimuth = (light.longitude as f32).to_radians();

    Vec3::new(
        azimuth.cos() * elevation.cos(),
        elevation.sin(),
        azimuth.sin() * elevation.cos(),
    )
    .normalize()
}

/// The baked directional (sun) illuminance. Outdoor maps mirror
/// `lighting.rs::calculate_global_lux` (RSW-derived, read at EV100 10.5). Indoor
/// maps pin a dim interior value read at EV100 9.7.
pub(crate) fn sun_illuminance(light: &RswLight, indoor: bool) -> f32 {
    if indoor {
        return INDOOR_SUN_LUX;
    }
    let diffuse_intensity = light.diffuse.iter().fold(0.0f32, |acc, &x| acc.max(x));
    SUN_MAX_LUX * diffuse_intensity * light.opacity
}

/// Candela for a point light, chosen so Bevy's importer (`lumens = candela *
/// 4 * pi`) lands on the native lumens from `lighting.rs::spawn_point_light`.
/// Indoor maps use the boosted lux-at-half-range so interiors read at EV100 9.7.
pub(crate) fn point_light_candela(light: &RswLightObj, indoor: bool) -> f32 {
    let lux_at_half = if indoor {
        INDOOR_POINT_LIGHT_LUX_AT_HALF_RANGE
    } else {
        POINT_LIGHT_LUX_AT_HALF_RANGE
    };
    let half_range = 0.5 * light.range;
    lux_at_half * half_range * half_range
}

/// The baked point-light cutoff range. Indoor maps widen it so boosted
/// interior lights still reach the far walls.
pub fn point_light_range(light: &RswLightObj, indoor: bool) -> f32 {
    if indoor {
        light.range * INDOOR_POINT_LIGHT_RANGE_SCALE
    } else {
        light.range
    }
}

/// The RSW objects that become nodes, in RSW order.
///
/// Rangeless/fileless sounds and models with no filename are dropped, exactly
/// as `map_sounds.rs::spawn_map_sounds` and `models.rs::load_rsm_assets` drop
/// them at runtime; emitting them would only create nodes the runtime has to
/// ignore again. Validation walks the very same list.
pub fn emitted_objects(world: &RoWorld) -> impl Iterator<Item = &RswObject> {
    world
        .objects
        .iter()
        .filter(|object| match object {
            RswObject::Sound(sound) => sound.range > 0.0 && !sound.wav_file.is_empty(),
            RswObject::Model(model) => !model.filename.is_empty(),
            _ => true,
        })
        .filter(|object| is_representable(object))
}

/// Whether every number this object contributes to the glb is a real number.
///
/// Retail data is not always sane. `t_garden.rsw` places four
/// `ilusion\waterpond_s_01.rsm2` props whose position, rotation and scale are
/// literal NaN; `gl_step.rsw` has a light at a NaN height; `ecl_in01.rsw` has
/// three nameless lights of range 7.2e22, whose candela overflows to infinity.
/// None of that describes anything the runtime could place, and glTF cannot
/// even carry it -- JSON has no NaN or infinity, so `serde_json` writes `null`
/// where a number belongs and the file no longer parses as glTF.
///
/// So such an object is dropped like the other unplaceable records above. The
/// check covers the values actually written, derived ones included, rather
/// than the raw fields alone: `ecl_in01`'s ranges are finite, and only the
/// candela computed from them is not.
pub fn is_representable(object: &RswObject) -> bool {
    let numbers: Vec<f32> = match object {
        RswObject::Model(model) => model
            .position
            .iter()
            .chain(&model.rotation)
            .chain(&model.scale)
            .copied()
            .chain([model.anim_speed])
            .collect(),
        RswObject::Light(light) => light
            .position
            .iter()
            .chain(&light.color)
            .copied()
            .chain([light.range, point_light_candela(light, false)])
            .collect(),
        RswObject::Sound(sound) => sound
            .position
            .iter()
            .copied()
            .chain([sound.volume, sound.range, sound.cycle])
            .collect(),
        RswObject::Effect(effect) => effect
            .position
            .iter()
            .chain(&effect.params)
            .copied()
            .chain([effect.emit_speed])
            .collect(),
    };

    numbers.iter().all(|value| value.is_finite())
}

/// The RSW name of an object, for reporting.
pub fn object_name(object: &RswObject) -> &str {
    match object {
        RswObject::Model(model) => &model.name,
        RswObject::Light(light) => &light.name,
        RswObject::Sound(sound) => &sound.name,
        RswObject::Effect(effect) => &effect.name,
    }
}

pub fn lif_audio(sound: &RswSound) -> lif::LifAudio {
    lif::LifAudio {
        name: sound.name.clone(),
        file: format!("ro://data/wav/{}", to_forward_slashes(&sound.wav_file)),
        volume: sound.volume,
        range: sound.range,
        cycle: sound.cycle,
    }
}

pub fn lif_effect(effect: &RswEffect) -> lif::LifEffect {
    lif::LifEffect {
        name: effect.name.clone(),
        effect_type: effect.effect_type,
        emit_speed: effect.emit_speed,
        params: effect.params,
    }
}

/// `converted` holds the models the per-map closure already turned into a
/// library glb. Every returned reference ends in `.glb`; a native fallback is
/// rejected rather than emitted.
pub fn lif_prop(model: &RswModel, converted: &HashSet<String>) -> anyhow::Result<lif::LifProp> {
    if !converted.contains(&model.filename) {
        anyhow::bail!("prop '{}' was not converted to a .glb", model.filename);
    }
    Ok(lif::LifProp {
        model: format!(
            "ro://models/{}",
            crate::converters::model::glb_relative_path(&model.filename)
        ),
        anim_type: model.anim_type,
        anim_speed: model.anim_speed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::map::fixtures::{TEXTURE, mini_ground, mini_world, raw_gat, textures};
    use crate::converters::map::terrain::build_terrain;

    fn retail_document() -> MapDocument {
        let ground = mini_ground();
        let world = mini_world();
        let gat = raw_gat(ground.width * 2, ground.height * 2);
        let primitives = build_terrain(&ground).expect("terrain");
        let textures = textures();
        let converted = HashSet::from(["prontera\\tree01.rsm".to_string()]);
        MapDocument::from_retail(RetailInputs {
            map_name: "mini",
            ground: &ground,
            world: &world,
            primitives: &primitives,
            textures: &textures,
            gat_bytes: &gat,
            gnd_bytes: b"gnd-bytes",
            rsw_bytes: b"rsw-bytes",
            converted_models: &converted,
            indoor: false,
        })
        .expect("document")
        .0
    }

    #[test]
    fn from_retail_keeps_the_light_math_and_payloads() {
        let document = retail_document();

        let MapLight::Sun { lux, .. } = document.lights[0] else {
            panic!("first light is the sun");
        };
        assert_eq!(lux, 4500.0);
        let MapLight::Point {
            translation,
            candela,
            range,
            ..
        } = document.lights[1]
        else {
            panic!("second light is the point light");
        };
        assert_eq!(candela, 500.0 * 20.0 * 20.0);
        assert_eq!(range, 40.0);
        assert_eq!(translation, Vec3::new(20.0, -5.0, 30.0));

        assert_eq!(document.sounds[0].translation, Vec3::new(11.0, -2.0, 13.0));
        assert_eq!(document.effects[0].translation, Vec3::new(14.0, -6.0, 18.0));
        assert_eq!(document.props[0].translation, Vec3::new(17.0, -8.0, 19.0));
        assert_eq!(document.props[0].scale, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(
            document.props[0].payload.model,
            "ro://models/prontera/tree01.glb"
        );
        assert_eq!(document.terrain.len(), 1);
        assert_eq!(document.textures[0].name, TEXTURE);
    }

    #[test]
    fn validate_rejects_an_empty_name_or_one_that_is_not_a_path_segment() {
        let mut document = retail_document();
        document.name = String::new();
        assert!(document.validate().is_err());

        document.name = "Not A Map".to_string();
        assert!(document.validate().is_err());

        document.name = "maps/prontera".to_string();
        assert!(document.validate().is_err());
    }

    #[test]
    fn validate_accepts_the_retail_name_alphabet() {
        let mut document = retail_document();

        for name in ["prontera_02", "1@pump", "pvp_y_1-2", "2@tower"] {
            document.name = name.to_string();
            assert!(
                document.validate().is_ok(),
                "retail map name '{name}' must be accepted"
            );
        }
    }

    #[test]
    fn validate_rejects_gat_dimensions_that_disagree_with_their_cell_count() {
        let mut document = retail_document();
        document.gat.width += 1;

        let errors = document.validate().expect_err("dims must match cells");
        assert!(errors.iter().any(|error| error.contains("GAT declares")));
    }

    #[test]
    fn validate_rejects_a_spot_whose_inner_angle_exceeds_its_outer_angle() {
        let mut document = retail_document();
        document.lights.push(MapLight::Spot {
            translation: Vec3::new(4.0, 6.0, -4.0),
            rotation: Quat::IDENTITY,
            color: [1.0; 3],
            candela: 50.0,
            range: 20.0,
            inner_angle: 0.8,
            outer_angle: 0.6,
        });

        let errors = document.validate().expect_err("the cone must be ordered");
        assert!(
            errors
                .iter()
                .any(|error| error.contains("spot light inner"))
        );
    }

    #[test]
    fn validate_rejects_a_terrain_texture_without_a_texture_ref() {
        let mut document = retail_document();
        document.textures.clear();

        let errors = document.validate().expect_err("textures must exist");
        assert!(errors.iter().any(|error| error.contains("no TextureRef")));
    }
}
