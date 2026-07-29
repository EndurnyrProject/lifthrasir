//! Assembles the unified per-map `.glb`: terrain primitives, per-texture
//! materials, `KHR_lights_punctual` lights, RSW emitter/prop nodes and the
//! `LIF_*` root extensions, all under one identity-transform root node.
//!
//! # Coordinate convention
//!
//! The runtime's world is -Y-up; glTF is Y-up. The runtime spawns this scene
//! under a single root fix of 180 degrees about X, so every position, normal
//! and node transform written here is pre-rotated by that same rotation --
//! which is its own inverse, so `(x, y, z)` is stored as `(x, -y, -z)` and a
//! node rotation `q` is stored as `FIX * q`. Applying the runtime root fix to
//! the imported data therefore reproduces the native path's world values
//! exactly, and because the fix is a proper rotation the glb is also right
//! way up in a stock glTF viewer.
//!
//! This assumes Bevy's experimental `GltfLoaderSettings::convert_coordinates`
//! stays at its default (off); enabling it would add a second rotation.

use crate::converters::gltf_out::{
    BinChunk, GeometryAttributes, extras_for, glb_container, push_geometry_primitive,
    push_image_and_texture, to_forward_slashes, to_gltf_quat, to_gltf_vec,
};
use crate::converters::map::terrain::TerrainPrimitive;
use crate::converters::map::textures::TextureOut;
use crate::converters::map::water;
use anyhow::Context;
use glam::{Mat3, Quat, Vec3};
use gltf_json as json;
use json::validation::{Checked::Valid, USize64};
use lifthrasir_data::lif;
use ro_formats::{
    RoAltitude, RoGround, RoWorld, RswEffect, RswLight, RswLightObj, RswModel, RswObject, RswSound,
};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

pub use crate::converters::gltf_out::{ROOT_FIX, hash_hex};

/// Native terrain material constants, mirrored from
/// `game-engine/src/domain/world/terrain.rs`.
const TERRAIN_ROUGHNESS: f32 = 1.0;
const TERRAIN_METALLIC: f32 = 0.0;
const TERRAIN_ALPHA_THRESHOLD: f32 = 0.5;

/// Native lighting constants, mirrored from
/// `game-engine/src/presentation/rendering/lighting.rs`.
const SUN_MAX_LUX: f32 = 10_000.0;
const POINT_LIGHT_LUX_AT_HALF_RANGE: f32 = 500.0;

/// Everything the writer needs; the caller owns parsing and texture export.
pub struct MapGlbInputs<'a> {
    pub map_name: &'a str,
    pub ground: &'a RoGround,
    pub world: &'a RoWorld,
    pub primitives: &'a [TerrainPrimitive],
    pub textures: &'a [TextureOut],
    /// Raw source bytes, hashed into `LIF_map` for provenance. The GAT bytes
    /// are additionally embedded verbatim in the bin chunk.
    pub gat_bytes: &'a [u8],
    pub gnd_bytes: &'a [u8],
    pub rsw_bytes: &'a [u8],
    /// `RswModel::filename`s the per-map closure already converted to a
    /// library glb (Converted or Skipped) -- what `lif_prop` flips to
    /// `ro://models/...` instead of the native `ro://data/model/...` ref.
    pub converted_models: &'a HashSet<String>,
}

/// Port of `game-engine/src/utils/coordinates.rs::rsw_position_to_bevy`.
pub fn rsw_position_to_world(position: [f32; 3], map_width: f32, map_height: f32) -> Vec3 {
    Vec3::new(
        position[0] + (map_width * 5.0),
        position[1],
        position[2] + (map_height * 5.0),
    )
}

/// Port of `game-engine/src/utils/coordinates.rs::rsw_to_bevy_transform`
/// (mat4.rotateZ, then rotateX, then rotateY).
pub fn rsw_model_rotation(model: &RswModel) -> Quat {
    Quat::from_rotation_z(model.rotation[2].to_radians())
        * Quat::from_rotation_x(model.rotation[0].to_radians())
        * Quat::from_rotation_y(model.rotation[1].to_radians())
}

/// Port of Bevy's `Transform::look_to`, used by the native directional light.
fn look_to_rotation(direction: Vec3, up: Vec3) -> Quat {
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

/// Mirrors `lighting.rs::calculate_global_lux`.
fn sun_illuminance(light: &RswLight) -> f32 {
    let diffuse_intensity = light.diffuse.iter().fold(0.0f32, |acc, &x| acc.max(x));
    SUN_MAX_LUX * diffuse_intensity * light.opacity
}

/// Candela for a point light, chosen so Bevy's importer (`lumens = candela *
/// 4 * pi`) lands on the native lumens from `lighting.rs::spawn_point_light`.
fn point_light_candela(light: &RswLightObj) -> f32 {
    let half_range = 0.5 * light.range;
    POINT_LIGHT_LUX_AT_HALF_RANGE * half_range * half_range
}

/// Emits the terrain mesh (one primitive per ground texture) and returns its
/// node, or `None` when the map has no ground geometry at all.
fn build_terrain_node(
    root: &mut json::Root,
    bin: &mut BinChunk,
    primitives: &[TerrainPrimitive],
    materials: &BTreeMap<String, json::Index<json::Material>>,
) -> anyhow::Result<Option<json::Index<json::Node>>> {
    let mut mesh_primitives = Vec::with_capacity(primitives.len());

    for primitive in primitives {
        if primitive.positions.is_empty() {
            continue;
        }
        mesh_primitives.push(build_primitive(root, bin, primitive, materials)?);
    }

    if mesh_primitives.is_empty() {
        return Ok(None);
    }

    let mesh = json::Index::push(
        &mut root.meshes,
        json::Mesh {
            primitives: mesh_primitives,
            name: Some("terrain".to_string()),
            weights: None,
            extensions: None,
            extras: Default::default(),
        },
    );

    Ok(Some(json::Index::push(
        &mut root.nodes,
        json::Node {
            mesh: Some(mesh),
            name: Some("terrain".to_string()),
            ..Default::default()
        },
    )))
}

fn build_primitive(
    root: &mut json::Root,
    bin: &mut BinChunk,
    primitive: &TerrainPrimitive,
    materials: &BTreeMap<String, json::Index<json::Material>>,
) -> anyhow::Result<json::mesh::Primitive> {
    let material = *materials.get(&primitive.texture).with_context(|| {
        format!(
            "terrain primitive references texture '{}' with no exported PNG",
            primitive.texture
        )
    })?;

    let positions: Vec<Vec3> = primitive
        .positions
        .iter()
        .map(|p| to_gltf_vec(Vec3::from(*p)))
        .collect();
    let normals: Vec<Vec3> = primitive
        .normals
        .iter()
        .map(|n| to_gltf_vec(Vec3::from(*n)))
        .collect();

    push_geometry_primitive(
        root,
        bin,
        &format!("terrain primitive '{}'", primitive.texture),
        &GeometryAttributes {
            positions: &positions,
            normals: &normals,
            colors: Some(&primitive.colors),
            uvs: &primitive.uvs,
            uv1: None,
            indices: &primitive.indices,
        },
        material,
    )
}

/// One image + texture + material per exported ground texture, keyed by the
/// GND texture name the primitives reference.
///
/// The sampler mirrors the runtime's terrain sampling
/// (`utils/mipmap.rs::apply_anisotropic_sampler`: linear min/mag/mipmap,
/// clamp-to-edge). Two native `StandardMaterial` fields have no glTF core
/// equivalent and are left to the runtime handler: `reflectance: 0.0` (glTF
/// pins F0 at 4%) and the anisotropic filtering level. `doubleSided` is the
/// closest expression of the native `cull_mode: None`, and additionally turns
/// on Bevy's back-face normal flipping.
fn build_materials(
    root: &mut json::Root,
    textures: &[TextureOut],
) -> BTreeMap<String, json::Index<json::Material>> {
    let sampler = json::Index::push(
        &mut root.samplers,
        json::texture::Sampler {
            mag_filter: Some(Valid(json::texture::MagFilter::Linear)),
            min_filter: Some(Valid(json::texture::MinFilter::LinearMipmapLinear)),
            wrap_s: Valid(json::texture::WrappingMode::ClampToEdge),
            wrap_t: Valid(json::texture::WrappingMode::ClampToEdge),
            name: None,
            extensions: None,
            extras: Default::default(),
        },
    );

    textures
        .iter()
        .map(|texture| {
            let base_color_texture = push_image_and_texture(root, texture, Some(sampler));
            let material = json::Index::push(
                &mut root.materials,
                json::Material {
                    alpha_cutoff: Some(json::material::AlphaCutoff(TERRAIN_ALPHA_THRESHOLD)),
                    alpha_mode: Valid(json::material::AlphaMode::Mask),
                    double_sided: true,
                    name: Some(texture.source_name.clone()),
                    pbr_metallic_roughness: json::material::PbrMetallicRoughness {
                        base_color_texture: Some(base_color_texture),
                        metallic_factor: json::material::StrengthFactor(TERRAIN_METALLIC),
                        roughness_factor: json::material::StrengthFactor(TERRAIN_ROUGHNESS),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );

            (texture.source_name.clone(), material)
        })
        .collect()
}

type KhrLight = json::extensions::scene::khr_lights_punctual::Light;
type KhrLightType = json::extensions::scene::khr_lights_punctual::Type;

fn push_light(root: &mut json::Root, light: KhrLight) -> json::Index<KhrLight> {
    let lights: &mut Vec<KhrLight> = root.as_mut();
    json::Index::push(lights, light)
}

fn light_node_extensions(light: json::Index<KhrLight>) -> json::extensions::scene::Node {
    json::extensions::scene::Node {
        khr_lights_punctual: Some(
            json::extensions::scene::khr_lights_punctual::KhrLightsPunctual { light },
        ),
        others: Default::default(),
    }
}

/// The RSW sun as a `KHR_lights_punctual` directional light. glTF directional
/// lights shine along their node's local -Z, which is also Bevy's
/// `Transform::forward`, so the node carries the native `looking_to` rotation
/// pushed through the root fix.
fn build_sun_node(root: &mut json::Root, light: &RswLight) -> json::Index<json::Node> {
    let index = push_light(
        root,
        KhrLight {
            color: light.diffuse,
            intensity: sun_illuminance(light),
            range: None,
            spot: None,
            type_: Valid(KhrLightType::Directional),
            name: Some("sun".to_string()),
            extensions: None,
            extras: Default::default(),
        },
    );

    let rotation = to_gltf_quat(look_to_rotation(sun_direction(light), Vec3::NEG_Y));

    json::Index::push(
        &mut root.nodes,
        json::Node {
            name: Some("sun".to_string()),
            rotation: Some(json::scene::UnitQuaternion(rotation.to_array())),
            extensions: Some(light_node_extensions(index)),
            ..Default::default()
        },
    )
}

fn build_point_light_node(
    root: &mut json::Root,
    light: &RswLightObj,
    map_width: f32,
    map_height: f32,
) -> json::Index<json::Node> {
    let index = push_light(
        root,
        KhrLight {
            color: light.color,
            intensity: point_light_candela(light),
            range: Some(light.range),
            spot: None,
            type_: Valid(KhrLightType::Point),
            name: Some(light.name.clone()),
            extensions: None,
            extras: Default::default(),
        },
    );

    let translation = to_gltf_vec(rsw_position_to_world(light.position, map_width, map_height));

    json::Index::push(
        &mut root.nodes,
        json::Node {
            name: Some(light.name.clone()),
            translation: Some(translation.to_array()),
            extensions: Some(light_node_extensions(index)),
            ..Default::default()
        },
    )
}

/// The RSW objects that become nodes, in RSW order.
///
/// Rangeless/fileless sounds and models with no filename are dropped, exactly
/// as `map_sounds.rs::spawn_map_sounds` and `models.rs::load_rsm_assets` drop
/// them at runtime; emitting them would only create nodes the runtime has to
/// ignore again. Validation walks the very same list.
pub fn emitted_objects(world: &RoWorld) -> impl Iterator<Item = &RswObject> {
    world.objects.iter().filter(|object| match object {
        RswObject::Sound(sound) => sound.range > 0.0 && !sound.wav_file.is_empty(),
        RswObject::Model(model) => !model.filename.is_empty(),
        _ => true,
    })
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
/// library glb; a member gets the `ro://models/...` ref, everyone else keeps
/// the native `.rsm` ref.
pub fn lif_prop(model: &RswModel, converted: &HashSet<String>) -> lif::LifProp {
    let model_ref = if converted.contains(&model.filename) {
        format!(
            "ro://models/{}",
            crate::converters::model::glb_relative_path(&model.filename)
        )
    } else {
        format!("ro://data/model/{}", to_forward_slashes(&model.filename))
    };
    lif::LifProp {
        model: model_ref,
        anim_type: model.anim_type,
        anim_speed: model.anim_speed,
    }
}

/// Lights, sound/effect emitters and prop references, in RSW order.
fn build_object_nodes(
    root: &mut json::Root,
    world: &RoWorld,
    map_width: f32,
    map_height: f32,
    converted_models: &HashSet<String>,
) -> anyhow::Result<Vec<json::Index<json::Node>>> {
    let mut nodes = Vec::new();

    for object in emitted_objects(world) {
        let node = match object {
            RswObject::Light(light) => build_point_light_node(root, light, map_width, map_height),
            RswObject::Sound(sound) => emitter_node(
                root,
                &sound.name,
                rsw_position_to_world(sound.position, map_width, map_height),
                extras_for(lif::EXTRAS_AUDIO, &lif_audio(sound))?,
            ),
            RswObject::Effect(effect) => emitter_node(
                root,
                &effect.name,
                rsw_position_to_world(effect.position, map_width, map_height),
                extras_for(lif::EXTRAS_EFFECT, &lif_effect(effect))?,
            ),
            RswObject::Model(model) => {
                build_prop_node(root, model, map_width, map_height, converted_models)?
            }
        };
        nodes.push(node);
    }

    Ok(nodes)
}

fn emitter_node(
    root: &mut json::Root,
    name: &str,
    world_position: Vec3,
    extras: json::Extras,
) -> json::Index<json::Node> {
    json::Index::push(
        &mut root.nodes,
        json::Node {
            name: Some(name.to_string()),
            translation: Some(to_gltf_vec(world_position).to_array()),
            extras,
            ..Default::default()
        },
    )
}

/// Prop node with the RSW placement fully baked, so the runtime only parents
/// the loaded RSM to it.
fn build_prop_node(
    root: &mut json::Root,
    model: &RswModel,
    map_width: f32,
    map_height: f32,
    converted_models: &HashSet<String>,
) -> anyhow::Result<json::Index<json::Node>> {
    let translation = to_gltf_vec(rsw_position_to_world(model.position, map_width, map_height));
    let rotation = to_gltf_quat(rsw_model_rotation(model));
    let extras = extras_for(lif::EXTRAS_PROP, &lif_prop(model, converted_models))?;

    Ok(json::Index::push(
        &mut root.nodes,
        json::Node {
            name: Some(model.name.clone()),
            translation: Some(translation.to_array()),
            rotation: Some(json::scene::UnitQuaternion(rotation.to_array())),
            scale: Some(model.scale),
            extras,
            ..Default::default()
        },
    ))
}

/// Assemble and write `<out_path>` as a binary glTF.
pub fn write_glb(out_path: &Path, inputs: &MapGlbInputs) -> anyhow::Result<()> {
    let mut root = json::Root {
        asset: json::Asset {
            generator: Some("ro-to-lifthrasir-cli".to_string()),
            version: "2.0".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bin = BinChunk::default();

    let materials = build_materials(&mut root, inputs.textures);
    let mut children = Vec::new();
    children.extend(build_terrain_node(
        &mut root,
        &mut bin,
        inputs.primitives,
        &materials,
    )?);

    let map_width = inputs.ground.width as f32;
    let map_height = inputs.ground.height as f32;
    children.push(build_sun_node(&mut root, &inputs.world.light));
    children.extend(build_object_nodes(
        &mut root,
        inputs.world,
        map_width,
        map_height,
        inputs.converted_models,
    )?);

    let scene_root = json::Index::push(
        &mut root.nodes,
        json::Node {
            children: Some(children),
            name: Some(inputs.map_name.to_string()),
            ..Default::default()
        },
    );
    let scene = json::Index::push(
        &mut root.scenes,
        json::Scene {
            nodes: vec![scene_root],
            name: Some(inputs.map_name.to_string()),
            extensions: None,
            extras: Default::default(),
        },
    );
    root.scene = Some(scene);

    build_root_extensions(&mut root, &mut bin, inputs)?;

    root.buffers.push(json::Buffer {
        byte_length: USize64::from(bin.data.len()),
        name: None,
        uri: None,
        extensions: None,
        extras: Default::default(),
    });
    root.buffer_views = std::mem::take(&mut bin.views);

    let json_bytes = json::serialize::to_vec(&root).context("serializing glTF JSON")?;
    std::fs::write(out_path, glb_container(&json_bytes, &bin.data))
        .with_context(|| format!("writing {}", out_path.display()))?;

    Ok(())
}

/// Root extensions: `LIF_map` always, `LIF_gat` with the source `.gat` bytes
/// embedded verbatim in the bin chunk, and `LIF_water` only when the RSW
/// declares a water level (`level == 0.0` is how RSW says "no water" -- the
/// same test `water.rs::load_water_system` makes).
fn build_root_extensions(
    root: &mut json::Root,
    bin: &mut BinChunk,
    inputs: &MapGlbInputs,
) -> anyhow::Result<()> {
    let altitude = RoAltitude::from_bytes(inputs.gat_bytes)
        .with_context(|| format!("parsing GAT of map '{}'", inputs.map_name))?;
    let gat_view = bin.push_view(inputs.gat_bytes, None);

    let map = lif::LifMap {
        format_version: lif::FORMAT_VERSION,
        rsw_hash: hash_hex(inputs.rsw_bytes),
        gnd_hash: hash_hex(inputs.gnd_bytes),
        gat_hash: hash_hex(inputs.gat_bytes),
        ambient_color: inputs.world.light.ambient,
        no_shade_tint: lif::no_shade_tint(inputs.world.light.ambient, inputs.world.light.diffuse),
    };
    let gat = lif::LifGat {
        width: altitude.width,
        height: altitude.height,
        buffer_view: gat_view.value() as u32,
    };

    let extensions = root.extensions.get_or_insert_with(Default::default);
    extensions
        .others
        .insert(lif::EXTENSION_MAP.to_string(), serde_json::to_value(&map)?);
    extensions
        .others
        .insert(lif::EXTENSION_GAT.to_string(), serde_json::to_value(gat)?);

    root.extensions_used = vec![
        "KHR_lights_punctual".to_string(),
        lif::EXTENSION_MAP.to_string(),
        lif::EXTENSION_GAT.to_string(),
    ];

    if inputs.world.water.level != 0.0 {
        let width = inputs.ground.width as usize;
        let height = inputs.ground.height as usize;
        let mask = lif::encode_water_mask(
            &water::select_water_tiles(inputs.ground, &inputs.world.water),
            width,
            height,
        );
        let mask_view = bin.push_view(&mask, None);
        let params = &inputs.world.water;
        let water = lif::LifWater {
            level: params.level,
            water_type: params.water_type,
            wave_height: params.wave_height,
            wave_speed: params.wave_speed,
            wave_pitch: params.wave_pitch,
            anim_speed: params.anim_speed,
            width: inputs.ground.width,
            height: inputs.ground.height,
            buffer_view: mask_view.value() as usize,
        };
        root.extensions
            .get_or_insert_with(Default::default)
            .others
            .insert(
                lif::EXTENSION_WATER.to_string(),
                serde_json::to_value(water)?,
            );
        root.extensions_used.push(lif::EXTENSION_WATER.to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::map::fixtures::{
        TEXTURE, mini_ground, mini_world, raw_gat, textures, write_fixture, write_fixture_png,
    };
    use crate::converters::map::terrain::build_terrain;
    use ro_formats::CELL_SIZE;
    use std::f32::consts::PI;

    #[test]
    fn writes_a_glb_that_reimports_with_the_expected_terrain_attributes() {
        let fixture = write_fixture();

        let (document, buffers, images) = gltf::import(&fixture.path).expect("reimport");

        assert_eq!(buffers.len(), 1);
        assert_eq!(images.len(), 1);

        let scene = document.default_scene().expect("default scene");
        let roots: Vec<_> = scene.nodes().collect();
        assert_eq!(roots.len(), 1, "scene must have a single root node");
        assert_eq!(roots[0].transform().matrix(), IDENTITY_MATRIX);

        let mesh = roots[0]
            .children()
            .find_map(|node| node.mesh())
            .expect("terrain mesh");
        let primitives: Vec<_> = mesh.primitives().collect();
        assert_eq!(primitives.len(), 1);

        let semantics: Vec<String> = primitives[0]
            .attributes()
            .map(|(semantic, _)| semantic.to_string())
            .collect();
        assert!(semantics.contains(&"POSITION".to_string()));
        assert!(semantics.contains(&"NORMAL".to_string()));
        assert!(semantics.contains(&"COLOR_0".to_string()));
        assert!(semantics.contains(&"TEXCOORD_0".to_string()));
        assert!(
            !semantics.iter().any(|s| s == "TEXCOORD_1"),
            "no lightmap UVs until Phase 3: {semantics:?}"
        );

        let material = primitives[0].material();
        assert_eq!(material.alpha_mode(), gltf::material::AlphaMode::Mask);
        assert_eq!(material.alpha_cutoff(), Some(TERRAIN_ALPHA_THRESHOLD));
        assert!(material.double_sided());
        let pbr = material.pbr_metallic_roughness();
        assert_eq!(pbr.metallic_factor(), TERRAIN_METALLIC);
        assert_eq!(pbr.roughness_factor(), TERRAIN_ROUGHNESS);
        assert!(pbr.base_color_texture().is_some());
    }

    const IDENTITY_MATRIX: [[f32; 4]; 4] = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    fn scene_root(document: &gltf::Document) -> gltf::Node<'_> {
        document
            .default_scene()
            .expect("default scene")
            .nodes()
            .next()
            .expect("scene root")
    }

    fn extras_value(node: &gltf::Node) -> Option<serde_json::Value> {
        node.extras()
            .as_ref()
            .map(|raw| serde_json::from_str(raw.get()).expect("node extras json"))
    }

    fn node_with_extras<'a>(root: &gltf::Node<'a>, key: &str) -> gltf::Node<'a> {
        root.children()
            .find(|node| extras_value(node).is_some_and(|value| value.get(key).is_some()))
            .unwrap_or_else(|| panic!("no node carries '{key}' extras"))
    }

    fn node_translation(node: &gltf::Node) -> Vec3 {
        let (translation, _, _) = node.transform().decomposed();
        Vec3::from(translation)
    }

    fn assert_close(actual: Vec3, expected: Vec3) {
        assert!(
            (actual - expected).length() < 1e-4,
            "expected {expected:?}, got {actual:?}"
        );
    }

    /// The whole point of the convention: the runtime applies exactly one root
    /// fix, and what comes out is the native mesh, vertex for vertex.
    #[test]
    fn a_terrain_vertex_round_trips_to_its_native_world_position() {
        let fixture = write_fixture();
        let native = build_terrain(&fixture.ground).expect("terrain").remove(0);

        let (document, buffers, _) = gltf::import(&fixture.path).expect("reimport");
        let root = scene_root(&document);
        let mesh = root
            .children()
            .find_map(|node| node.mesh())
            .expect("terrain mesh");
        let primitive = mesh.primitives().next().expect("primitive");
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
        let imported: Vec<[f32; 3]> = reader.read_positions().expect("positions").collect();

        assert_eq!(imported.len(), native.positions.len());
        assert_eq!(native.positions[0], [0.0, -10.0, 0.0]);
        for (imported, native) in imported.iter().zip(&native.positions) {
            assert_close(ROOT_FIX * Vec3::from(*imported), Vec3::from(*native));
        }

        let imported_normals: Vec<[f32; 3]> = reader.read_normals().expect("normals").collect();
        for (imported, native) in imported_normals.iter().zip(&native.normals) {
            assert_close(ROOT_FIX * Vec3::from(*imported), Vec3::from(*native));
        }
        assert_eq!(CELL_SIZE, 10.0);
    }

    #[test]
    fn lif_gat_buffer_view_holds_the_source_gat_bytes_verbatim() {
        let fixture = write_fixture();

        let (document, buffers, _) = gltf::import(&fixture.path).expect("reimport");
        let root_json = document.into_json();
        let extensions = root_json.extensions.as_ref().expect("root extensions");
        let gat: lif::LifGat =
            serde_json::from_value(extensions.others[lif::EXTENSION_GAT].clone()).expect("LIF_gat");

        let view = &root_json.buffer_views[gat.buffer_view as usize];
        let start = view.byte_offset.expect("byte offset").0 as usize;
        let end = start + view.byte_length.0 as usize;
        let embedded = &buffers[0][start..end];

        assert_eq!(embedded, fixture.gat.as_slice());
        assert_eq!(gat.width, 4);
        assert_eq!(gat.height, 4);
        assert!(gat.validate(embedded).is_ok());
    }

    #[test]
    fn lif_water_buffer_view_holds_the_selected_gnd_mask() {
        let mut fixture = write_fixture();
        fixture.ground.surfaces[0].height = [12.0; 4];
        write_glb(&fixture.path, &fixture.inputs()).expect("rewrite glb");

        let (document, buffers, _) = gltf::import(&fixture.path).expect("reimport");
        let root_json = document.into_json();
        let extensions = root_json.extensions.as_ref().expect("root extensions");
        let water: lif::LifWater =
            serde_json::from_value(extensions.others[lif::EXTENSION_WATER].clone())
                .expect("LIF_water");

        let view = &root_json.buffer_views[water.buffer_view];
        let start = view.byte_offset.expect("byte offset").0 as usize;
        let end = start + view.byte_length.0 as usize;
        let embedded = &buffers[0][start..end];
        let width = fixture.ground.width as usize;
        let height = fixture.ground.height as usize;

        assert_eq!(water.width, fixture.ground.width);
        assert_eq!(water.height, fixture.ground.height);
        assert_eq!(embedded.len(), (width * height).div_ceil(8));
        assert_eq!(
            lif::decode_water_mask(embedded, width, height),
            crate::converters::map::water::select_water_tiles(
                &fixture.ground,
                &fixture.world.water
            )
        );
    }

    #[test]
    fn root_extensions_carry_map_and_water_metadata() {
        let fixture = write_fixture();

        let (document, _, _) = gltf::import(&fixture.path).expect("reimport");
        let root_json = document.into_json();
        let extensions = root_json.extensions.as_ref().expect("root extensions");

        let map: lif::LifMap =
            serde_json::from_value(extensions.others[lif::EXTENSION_MAP].clone()).expect("LIF_map");
        assert_eq!(map.format_version, lif::FORMAT_VERSION);
        assert_eq!(map.ambient_color, [0.25, 0.3, 0.35]);
        assert_eq!(map.gat_hash, hash_hex(&fixture.gat));
        assert_eq!(map.gnd_hash, hash_hex(b"gnd-bytes"));
        assert_eq!(map.rsw_hash, hash_hex(b"rsw-bytes"));

        let water: lif::LifWater =
            serde_json::from_value(extensions.others[lif::EXTENSION_WATER].clone())
                .expect("LIF_water");
        assert_eq!(water.level, fixture.world.water.level);
        assert_eq!(water.water_type, fixture.world.water.water_type);
        assert_eq!(water.wave_height, fixture.world.water.wave_height);
        assert_eq!(water.wave_speed, fixture.world.water.wave_speed);
        assert_eq!(water.wave_pitch, fixture.world.water.wave_pitch);
        assert_eq!(water.anim_speed, fixture.world.water.anim_speed);

        for name in [
            "KHR_lights_punctual",
            lif::EXTENSION_MAP,
            lif::EXTENSION_GAT,
            lif::EXTENSION_WATER,
        ] {
            assert!(
                root_json.extensions_used.iter().any(|used| used == name),
                "{name} missing from extensionsUsed"
            );
        }
        assert!(root_json.extensions_required.is_empty());
    }

    #[test]
    fn a_map_without_water_has_no_water_extension() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_fixture_png(dir.path(), "tex/grass01.png");
        let ground = mini_ground();
        let mut world = mini_world();
        world.water.level = 0.0;
        let path = dir.path().join("dry.glb");

        write_glb(
            &path,
            &MapGlbInputs {
                map_name: "dry",
                ground: &ground,
                world: &world,
                primitives: &build_terrain(&ground).expect("terrain"),
                textures: &textures(),
                gat_bytes: &raw_gat(4, 4),
                gnd_bytes: b"gnd",
                rsw_bytes: b"rsw",
                converted_models: &HashSet::new(),
            },
        )
        .expect("write glb");

        let (document, _, _) = gltf::import(&path).expect("reimport");
        let root_json = document.into_json();
        let extensions = root_json.extensions.as_ref().expect("root extensions");

        assert!(!extensions.others.contains_key(lif::EXTENSION_WATER));
        assert!(
            !root_json
                .extensions_used
                .iter()
                .any(|used| used == lif::EXTENSION_WATER)
        );
    }

    #[test]
    fn the_sun_node_points_where_the_native_directional_light_does() {
        let fixture = write_fixture();

        let (document, _, _) = gltf::import(&fixture.path).expect("reimport");
        let root = scene_root(&document);
        let (node, light) = root
            .children()
            .find_map(|node| {
                let light = node.light()?;
                matches!(light.kind(), gltf::khr_lights_punctual::Kind::Directional)
                    .then_some((node, light))
            })
            .expect("directional light");

        assert_eq!(light.color(), [0.9, 0.8, 0.7]);
        assert_eq!(light.intensity(), 4500.0);

        let (_, rotation, _) = node.transform().decomposed();
        let world_rotation = ROOT_FIX * Quat::from_array(rotation);

        // latitude 30 -> elevation 60, longitude 45 -> azimuth 45.
        let expected = Vec3::new(0.353_553_4, 0.866_025_4, 0.353_553_4);
        assert_close(world_rotation * Vec3::NEG_Z, expected);
        assert_close(sun_direction(&fixture.world.light), expected);
    }

    #[test]
    fn point_lights_keep_their_native_placement_and_lumens() {
        let fixture = write_fixture();

        let (document, _, _) = gltf::import(&fixture.path).expect("reimport");
        let root = scene_root(&document);
        let (node, light) = root
            .children()
            .find_map(|node| {
                let light = node.light()?;
                matches!(light.kind(), gltf::khr_lights_punctual::Kind::Point)
                    .then_some((node, light))
            })
            .expect("point light");

        assert_eq!(light.color(), [1.0, 0.5, 0.25]);
        assert_eq!(light.range(), Some(40.0));
        // Bevy's importer multiplies candela by 4*pi, landing on the native
        // lumens of `POINT_LIGHT_LUX_AT_HALF_RANGE * 4 * pi * half_range^2`.
        assert_eq!(light.intensity(), 500.0 * 20.0 * 20.0);
        assert_eq!(
            light.intensity() * 4.0 * PI,
            POINT_LIGHT_LUX_AT_HALF_RANGE * 4.0 * PI * 20.0 * 20.0
        );

        assert_close(
            ROOT_FIX * node_translation(&node),
            Vec3::new(20.0, -5.0, 30.0),
        );
    }

    #[test]
    fn sound_and_effect_nodes_carry_their_params_at_baked_positions() {
        let fixture = write_fixture();

        let (document, _, _) = gltf::import(&fixture.path).expect("reimport");
        let root = scene_root(&document);

        let audio_node = node_with_extras(&root, lif::EXTRAS_AUDIO);
        let audio: lif::LifAudio = serde_json::from_value(
            extras_value(&audio_node).expect("extras")[lif::EXTRAS_AUDIO].clone(),
        )
        .expect("lif_audio");
        assert_eq!(
            audio,
            lif::LifAudio {
                name: "water".to_string(),
                file: "ro://data/wav/effect/water.wav".to_string(),
                volume: 0.75,
                range: 30.0,
                cycle: 4.5,
            }
        );
        assert_close(
            ROOT_FIX * node_translation(&audio_node),
            Vec3::new(11.0, -2.0, 13.0),
        );

        let effect_node = node_with_extras(&root, lif::EXTRAS_EFFECT);
        let effect: lif::LifEffect = serde_json::from_value(
            extras_value(&effect_node).expect("extras")[lif::EXTRAS_EFFECT].clone(),
        )
        .expect("lif_effect");
        assert_eq!(
            effect,
            lif::LifEffect {
                name: "steam".to_string(),
                effect_type: 12,
                emit_speed: 0.25,
                params: [1.0, 2.0, 3.0, 4.0],
            }
        );
        assert_close(
            ROOT_FIX * node_translation(&effect_node),
            Vec3::new(14.0, -6.0, 18.0),
        );
    }

    #[test]
    fn prop_nodes_bake_the_full_rsw_placement() {
        let fixture = write_fixture();

        let (document, _, _) = gltf::import(&fixture.path).expect("reimport");
        let root = scene_root(&document);
        let node = node_with_extras(&root, lif::EXTRAS_PROP);

        let prop: lif::LifProp =
            serde_json::from_value(extras_value(&node).expect("extras")[lif::EXTRAS_PROP].clone())
                .expect("lif_prop");
        assert_eq!(prop.model, "ro://data/model/prontera/tree01.rsm");
        assert_eq!(prop.anim_type, 1);
        assert_eq!(prop.anim_speed, 2.0);

        let (translation, rotation, scale) = node.transform().decomposed();
        assert_close(
            ROOT_FIX * Vec3::from(translation),
            Vec3::new(17.0, -8.0, 19.0),
        );
        assert_eq!(scale, [1.0, 2.0, 3.0]);

        // mat4.rotateZ, then rotateX, then rotateY, per rsw_to_bevy_transform.
        let expected = Quat::from_rotation_z(30f32.to_radians())
            * Quat::from_rotation_x(10f32.to_radians())
            * Quat::from_rotation_y(20f32.to_radians());
        let world = ROOT_FIX * Quat::from_array(rotation);
        assert_close(world * Vec3::X, expected * Vec3::X);
        assert_close(world * Vec3::Y, expected * Vec3::Y);
        assert_close(world * Vec3::Z, expected * Vec3::Z);
        assert_eq!(fixture.world.objects.len(), 4);
    }

    #[test]
    fn lif_prop_flips_to_the_library_glb_ref_when_the_model_was_converted() {
        let world = mini_world();
        let model = world
            .objects
            .iter()
            .find_map(|object| match object {
                RswObject::Model(model) => Some(model),
                _ => None,
            })
            .expect("model object");

        let native = lif_prop(model, &HashSet::new());
        assert_eq!(native.model, "ro://data/model/prontera/tree01.rsm");

        let mut converted = HashSet::new();
        converted.insert(model.filename.clone());
        let flipped = lif_prop(model, &converted);
        assert_eq!(
            flipped.model,
            format!(
                "ro://models/{}",
                crate::converters::model::glb_relative_path(&model.filename)
            )
        );
    }

    #[test]
    fn a_primitive_referencing_an_unexported_texture_fails_loudly() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ground = mini_ground();

        let err = write_glb(
            &dir.path().join("mini.glb"),
            &MapGlbInputs {
                map_name: "mini",
                ground: &ground,
                world: &mini_world(),
                primitives: &build_terrain(&ground).expect("terrain"),
                textures: &[],
                gat_bytes: &raw_gat(4, 4),
                gnd_bytes: b"gnd",
                rsw_bytes: b"rsw",
                converted_models: &HashSet::new(),
            },
        )
        .expect_err("missing texture must fail");

        assert!(err.to_string().contains(TEXTURE), "unexpected error: {err}");
    }
}
