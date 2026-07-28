//! Assembles one prop `.glb` per RSM: a synthetic pre-rotated root node
//! wrapping the RSM node hierarchy, one primitive per (node, texture), the
//! native model materials, one standard glTF animation baked from the RSM
//! keyframes, and the `LIF_model` provenance stamp.
//!
//! # Coordinate convention
//!
//! Unlike the map writer, which pre-rotates every world-space value it emits,
//! this writer applies the root fix exactly once: the synthetic root node
//! carries `to_gltf_quat(Quat::IDENTITY)` and everything below it -- node TRS,
//! vertices, normals and keyframe values -- is written raw-local, verbatim
//! from the RSM. The runtime spawns the scene under a `ROOT_FIX` rotation, so
//! `ROOT_FIX * root_rotation` cancels to identity and each node reproduces its
//! native local transform exactly. Pushing the node values through the fix as
//! well would rotate them twice.

use crate::converters::gltf_out::{
    BinChunk, GeometryAttributes, accessor, f32_bytes, glb_container, push_geometry_primitive,
    push_image_and_texture, to_gltf_quat,
};
use crate::converters::map::textures::TextureOut;
use crate::converters::model::normalized::{
    AlphaMode, NormalizedMaterial, NormalizedModel, NormalizedNode, NormalizedPrimitive,
    NormalizedTrack,
};
use anyhow::{Context, bail};
use glam::{Quat, Vec3};
use gltf_json as json;
use json::validation::{Checked::Valid, USize64};
use lifthrasir_data::lif;
use std::path::Path;

/// Native model material constants, mirrored from
/// `game-engine/src/presentation/rendering/models.rs::create_model_materials_from_loaded_textures`.
const MODEL_ROUGHNESS: f32 = 1.0;
const MODEL_METALLIC: f32 = 0.0;
#[cfg(test)]
const MODEL_ALPHA_CUTOFF: f32 = 0.01;

/// The single animation a prop glb carries; the runtime plays
/// `GltfAssetLabel::Animation(0)`.
const ANIMATION_NAME: &str = "anim";

/// Assemble and write `<out_path>` as a binary glTF.
///
/// `textures` is index-aligned with `rsm.textures`; the caller owns exporting
/// the PNGs and computing each `relative_path` from the glb's own directory.
pub fn write_model_glb(
    out_path: &Path,
    model: &NormalizedModel,
    textures: &[TextureOut],
) -> anyhow::Result<()> {
    super::validate::validate_contract(model)?;
    let model_name = out_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .with_context(|| format!("model output path has no file stem: {}", out_path.display()))?
        .to_string();

    let mut root = json::Root {
        asset: json::Asset {
            generator: Some("ro-to-lifthrasir-cli".to_string()),
            version: "2.0".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bin = BinChunk::default();

    let materials = build_materials(&mut root, model, textures)?;
    let nodes = build_nodes(&mut root, &mut bin, model, &materials)?;
    build_animation(&mut root, &mut bin, model, &nodes)?;

    let children = model
        .roots
        .iter()
        .map(|index| {
            nodes
                .get(*index)
                .copied()
                .with_context(|| format!("model root index {index} is out of range"))
        })
        .collect::<anyhow::Result<_>>()?;

    let scene_root = json::Index::push(
        &mut root.nodes,
        json::Node {
            children: Some(children),
            name: Some(model_name.clone()),
            rotation: Some(json::scene::UnitQuaternion(
                to_gltf_quat(Quat::IDENTITY).to_array(),
            )),
            ..Default::default()
        },
    );
    let scene = json::Index::push(
        &mut root.scenes,
        json::Scene {
            nodes: vec![scene_root],
            name: Some(model_name),
            extensions: None,
            extras: Default::default(),
        },
    );
    root.scene = Some(scene);

    build_root_extensions(&mut root, &model.provenance.source_hash)?;

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

/// One image + texture + material per RSM texture index the geometry actually
/// references, keyed by that index.
///
/// Mirrors `create_model_materials_from_loaded_textures`: pure lambert
/// (`metallic 0`, `roughness 1`), `doubleSided` for the native `cull_mode:
/// None`, and either `MASK` at a 0.01 cutoff or -- when the RSM declares model
/// alpha -- `BLEND` with that alpha in `baseColorFactor`. `reflectance: 0.0`
/// has no glTF field and is left to the runtime observer.
///
/// The sampler is deliberately left unset: the glTF default is Bevy's default
/// image sampler, which is what the native path gets loading the BMP through
/// the asset server.
///
/// A `-1` texture id means the face referenced a slot the node does not have.
/// The native path answers that with a debug-colored fallback material; here it
/// is an untextured material carrying the very same flags, so the geometry
/// still renders (white) instead of vanishing, without baking debug colors
/// into the shipped asset.
fn build_materials(
    root: &mut json::Root,
    model: &NormalizedModel,
    textures: &[TextureOut],
) -> anyhow::Result<Vec<json::Index<json::Material>>> {
    if textures.len() != model.textures.len() {
        bail!(
            "model declares {} texture(s) but {} were exported",
            model.textures.len(),
            textures.len()
        );
    }

    model
        .materials
        .iter()
        .map(|material| {
            let source = material
                .texture
                .map(|index| {
                    textures.get(index).with_context(|| {
                        format!(
                            "model geometry references texture index {index} but only {} were exported",
                            textures.len()
                        )
                    })
                })
                .transpose()?;
            let base_color_texture =
                source.map(|texture| push_image_and_texture(root, texture, None));
            let name = source.map(|texture| texture.source_name.clone());
            Ok(json::Index::push(
                &mut root.materials,
                model_material(name, base_color_texture, material),
            ))
        })
        .collect()
}

fn model_material(
    name: Option<String>,
    base_color_texture: Option<json::texture::Info>,
    material: &NormalizedMaterial,
) -> json::Material {
    let (alpha_mode, alpha_cutoff) = match material.alpha {
        AlphaMode::Mask { cutoff } => (
            json::material::AlphaMode::Mask,
            Some(json::material::AlphaCutoff(cutoff)),
        ),
        AlphaMode::Blend => (json::material::AlphaMode::Blend, None),
    };

    json::Material {
        alpha_cutoff,
        alpha_mode: Valid(alpha_mode),
        double_sided: material.two_sided,
        name,
        pbr_metallic_roughness: json::material::PbrMetallicRoughness {
            // Native keeps `base_color: Color::WHITE` even when `rsm.alpha < 1`
            // and only flips the alpha mode to Blend (models.rs::
            // create_model_materials_from_loaded_textures) -- sameness with the
            // native render wins over the RSM's nominal alpha value.
            base_color_factor: json::material::PbrBaseColorFactor([1.0, 1.0, 1.0, 1.0]),
            base_color_texture,
            metallic_factor: json::material::StrengthFactor(MODEL_METALLIC),
            roughness_factor: json::material::StrengthFactor(MODEL_ROUGHNESS),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// One glTF node per `ModelNode`, in build order so the returned indices are
/// index-aligned with `build.nodes`, each carrying its raw-local TRS and, when
/// it has geometry, its own mesh.
fn build_nodes(
    root: &mut json::Root,
    bin: &mut BinChunk,
    model: &NormalizedModel,
    materials: &[json::Index<json::Material>],
) -> anyhow::Result<Vec<json::Index<json::Node>>> {
    let mut indices = Vec::with_capacity(model.nodes.len());

    for node in &model.nodes {
        let mesh = build_mesh(root, bin, node, materials)?;
        indices.push(json::Index::push(
            &mut root.nodes,
            json::Node {
                mesh,
                name: Some(node.name.clone()),
                translation: Some(node.translation),
                rotation: Some(json::scene::UnitQuaternion(node.rotation)),
                scale: Some(node.scale),
                ..Default::default()
            },
        ));
    }

    for (index, node) in model.nodes.iter().enumerate() {
        let Some(parent) = node.parent else {
            continue;
        };
        let child = indices[index];
        let parent = *indices
            .get(parent)
            .with_context(|| format!("node '{}' has an out-of-range parent", node.name))?;
        root.nodes[parent.value()]
            .children
            .get_or_insert_with(Default::default)
            .push(child);
    }

    Ok(indices)
}

fn build_mesh(
    root: &mut json::Root,
    bin: &mut BinChunk,
    node: &NormalizedNode,
    materials: &[json::Index<json::Material>],
) -> anyhow::Result<Option<json::Index<json::Mesh>>> {
    if node.primitives.is_empty() {
        return Ok(None);
    }

    let primitives = node
        .primitives
        .iter()
        .map(|primitive| build_primitive(root, bin, &node.name, primitive, materials))
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(Some(json::Index::push(
        &mut root.meshes,
        json::Mesh {
            primitives,
            name: Some(node.name.clone()),
            weights: None,
            extensions: None,
            extras: Default::default(),
        },
    )))
}

fn build_primitive(
    root: &mut json::Root,
    bin: &mut BinChunk,
    node_name: &str,
    primitive: &NormalizedPrimitive,
    materials: &[json::Index<json::Material>],
) -> anyhow::Result<json::mesh::Primitive> {
    let material = *materials
        .get(primitive.material)
        .with_context(|| format!("node '{node_name}' has no material for its geometry"))?;

    let positions: Vec<Vec3> = primitive.positions.iter().map(|p| Vec3::from(*p)).collect();
    let normals: Vec<Vec3> = primitive.normals.iter().map(|n| Vec3::from(*n)).collect();

    push_geometry_primitive(
        root,
        bin,
        &format!("node '{node_name}' material {}", primitive.material),
        &GeometryAttributes {
            positions: &positions,
            normals: &normals,
            colors: None,
            uvs: &primitive.uv0,
            indices: &primitive.indices,
        },
        material,
    )
}

/// The single `"anim"` animation: a linear translation and/or rotation channel
/// per keyframed node, with the native per-channel frame rescale baked into
/// the sampler input times.
///
/// A model with no keyframes at all, or one whose `anim_len` is zero, gets no
/// animation -- the runtime observer then finds no `AnimationPlayer` and
/// leaves the prop static, matching the native path.
fn build_animation(
    root: &mut json::Root,
    bin: &mut BinChunk,
    model: &NormalizedModel,
    nodes: &[json::Index<json::Node>],
) -> anyhow::Result<()> {
    if model.duration_ms <= 0.0 {
        return Ok(());
    }

    let mut samplers = Vec::new();
    let mut channels = Vec::new();

    for (index, node) in model.nodes.iter().enumerate() {
        let mut writer = TrackWriter {
            root,
            bin,
            samplers: &mut samplers,
            channels: &mut channels,
            node_name: &node.name,
            node: nodes[index],
        };
        writer.push(
            &node.translation_track,
            json::animation::Property::Translation,
            json::accessor::Type::Vec3,
            |value| value.to_vec(),
        )?;
        writer.push(
            &node.rotation_track,
            json::animation::Property::Rotation,
            json::accessor::Type::Vec4,
            |value| value.to_vec(),
        )?;
        writer.push(
            &node.scale_track,
            json::animation::Property::Scale,
            json::accessor::Type::Vec3,
            |value| value.to_vec(),
        )?;
    }

    if !channels.is_empty() {
        root.animations.push(json::Animation {
            channels,
            samplers,
            name: Some(ANIMATION_NAME.to_string()),
            extensions: None,
            extras: Default::default(),
        });
    }

    Ok(())
}

struct TrackWriter<'a> {
    root: &'a mut json::Root,
    bin: &'a mut BinChunk,
    samplers: &'a mut Vec<json::animation::Sampler>,
    channels: &'a mut Vec<json::animation::Channel>,
    node_name: &'a str,
    node: json::Index<json::Node>,
}

impl TrackWriter<'_> {
    fn push<T>(
        &mut self,
        track: &NormalizedTrack<T>,
        property: json::animation::Property,
        type_: json::accessor::Type,
        values: impl Fn(&T) -> Vec<f32>,
    ) -> anyhow::Result<()> {
        if track.keys.is_empty() {
            return Ok(());
        }

        let times: Vec<f32> = track.keys.iter().map(|key| key.time_ms / 1000.0).collect();
        if times.iter().any(|time| !time.is_finite())
            || times.windows(2).any(|pair| pair[1] <= pair[0])
        {
            bail!(
                "node '{}' has non-increasing or non-finite {property:?} keyframes",
                self.node_name
            );
        }
        let sampler = push_sampler(
            self.root,
            self.bin,
            self.samplers,
            &times,
            track.keys.iter().flat_map(|key| values(&key.value)),
            type_,
        );
        self.channels.push(channel(sampler, self.node, property));
        Ok(())
    }
}

fn push_sampler(
    root: &mut json::Root,
    bin: &mut BinChunk,
    samplers: &mut Vec<json::animation::Sampler>,
    times: &[f32],
    values: impl IntoIterator<Item = f32>,
    type_: json::accessor::Type,
) -> json::Index<json::animation::Sampler> {
    let input_view = bin.push_view(&f32_bytes(times.iter().copied()), None);
    let mut input_accessor = accessor(
        input_view,
        times.len(),
        json::accessor::ComponentType::F32,
        json::accessor::Type::Scalar,
    );
    input_accessor.min = Some(serde_json::json!([times
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min)]));
    input_accessor.max = Some(serde_json::json!([times
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max)]));
    let input = json::Index::push(&mut root.accessors, input_accessor);

    let output_view = bin.push_view(&f32_bytes(values), None);
    let output = json::Index::push(
        &mut root.accessors,
        accessor(
            output_view,
            times.len(),
            json::accessor::ComponentType::F32,
            type_,
        ),
    );

    json::Index::push(
        samplers,
        json::animation::Sampler {
            input,
            output,
            interpolation: Valid(json::animation::Interpolation::Linear),
            extensions: None,
            extras: Default::default(),
        },
    )
}

fn channel(
    sampler: json::Index<json::animation::Sampler>,
    node: json::Index<json::Node>,
    path: json::animation::Property,
) -> json::animation::Channel {
    json::animation::Channel {
        sampler,
        target: json::animation::Target {
            node,
            path: Valid(path),
            extensions: None,
            extras: Default::default(),
        },
        extensions: None,
        extras: Default::default(),
    }
}

fn build_root_extensions(root: &mut json::Root, rsm_hash: &str) -> anyhow::Result<()> {
    let model = lif::LifModel {
        format_version: lif::FORMAT_VERSION,
        rsm_hash: rsm_hash.to_string(),
    };

    root.extensions
        .get_or_insert_with(Default::default)
        .others
        .insert(
            lif::EXTENSION_MODEL.to_string(),
            serde_json::to_value(&model)?,
        );
    root.extensions_used = vec![lif::EXTENSION_MODEL.to_string()];

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::gltf_out::ROOT_FIX;
    use crate::converters::model::fixtures::{
        ModelFixture, TEXTURES, animated_rsm, fixture_dir, textured_rsm, textures, write_fixture,
    };
    use crate::converters::model::mesh::build_model;

    fn assert_close(actual: Vec3, expected: Vec3) {
        assert!(
            (actual - expected).length() < 1e-5,
            "expected {expected:?}, got {actual:?}"
        );
    }

    fn scene_root(document: &gltf::Document) -> gltf::Node<'_> {
        document
            .default_scene()
            .expect("default scene")
            .nodes()
            .next()
            .expect("scene root")
    }

    fn node_named<'a>(document: &'a gltf::Document, name: &str) -> gltf::Node<'a> {
        document
            .nodes()
            .find(|node| node.name() == Some(name))
            .unwrap_or_else(|| panic!("no node named '{name}'"))
    }

    #[test]
    fn the_synthetic_root_undoes_the_runtime_root_fix_for_every_node_below_it() {
        let fixture = write_fixture();
        let (document, buffers, images) = gltf::import(&fixture.path).expect("reimport");

        assert_eq!(images.len(), TEXTURES.len());

        let root = scene_root(&document);
        assert_eq!(root.name(), Some("tree01"));
        assert_eq!(root.children().count(), 1);

        let (_, root_rotation, _) = root.transform().decomposed();
        let root_rotation = Quat::from_array(root_rotation);

        let main = node_named(&document, "main");
        let primitive = main
            .mesh()
            .expect("main mesh")
            .primitives()
            .next()
            .expect("primitive");
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
        let imported: Vec<[f32; 3]> = reader.read_positions().expect("positions").collect();

        let native = &fixture.model.nodes[0].primitives[0].positions;
        assert_eq!(imported.len(), native.len());
        for (imported, native) in imported.iter().zip(native) {
            assert_close(
                ROOT_FIX * (root_rotation * Vec3::from(*imported)),
                Vec3::from(*native),
            );
        }

        let normals: Vec<[f32; 3]> = reader.read_normals().expect("normals").collect();
        for (imported, native) in normals
            .iter()
            .zip(&fixture.model.nodes[0].primitives[0].normals)
        {
            assert_close(
                ROOT_FIX * (root_rotation * Vec3::from(*imported)),
                Vec3::from(*native),
            );
        }
    }

    #[test]
    fn nodes_keep_their_raw_local_trs_and_parent_links() {
        let fixture = write_fixture();
        let (document, _, _) = gltf::import(&fixture.path).expect("reimport");

        let main = node_named(&document, "main");
        let child = node_named(&document, "child");

        for (node, built) in [
            (&main, &fixture.model.nodes[0]),
            (&child, &fixture.model.nodes[1]),
        ] {
            let (translation, rotation, scale) = node.transform().decomposed();
            assert_eq!(translation, built.translation);
            assert_eq!(scale, built.scale);
            assert_close(
                Quat::from_array(rotation) * Vec3::ONE,
                Quat::from_array(built.rotation) * Vec3::ONE,
            );
        }

        let child_names: Vec<&str> = main.children().filter_map(|node| node.name()).collect();
        assert_eq!(child_names, vec!["child"]);
        assert_eq!(child.children().count(), 0);
    }

    #[test]
    fn one_primitive_per_node_texture_with_the_expected_semantics() {
        let fixture = write_fixture();
        let (document, buffers, _) = gltf::import(&fixture.path).expect("reimport");

        let main = node_named(&document, "main");
        let primitives: Vec<_> = main.mesh().expect("main mesh").primitives().collect();
        assert_eq!(primitives.len(), 2, "one per referenced texture id");
        assert_eq!(
            node_named(&document, "child")
                .mesh()
                .expect("child mesh")
                .primitives()
                .count(),
            1
        );

        let semantics: Vec<String> = primitives[0]
            .attributes()
            .map(|(semantic, _)| semantic.to_string())
            .collect();
        assert_eq!(
            semantics,
            vec![
                "POSITION".to_string(),
                "NORMAL".to_string(),
                "TEXCOORD_0".to_string()
            ]
        );

        let reader = primitives[0].reader(|buffer| Some(&buffers[buffer.index()]));
        let indices: Vec<u32> = reader.read_indices().expect("indices").into_u32().collect();
        assert_eq!(indices, fixture.model.nodes[0].primitives[0].indices);
    }

    #[test]
    fn an_opaque_model_masks_at_the_native_cutoff() {
        let fixture = write_fixture();
        let (document, _, _) = gltf::import(&fixture.path).expect("reimport");

        let material = node_named(&document, "main")
            .mesh()
            .expect("mesh")
            .primitives()
            .next()
            .expect("primitive")
            .material();

        assert_eq!(material.alpha_mode(), gltf::material::AlphaMode::Mask);
        assert_eq!(material.alpha_cutoff(), Some(MODEL_ALPHA_CUTOFF));
        assert!(material.double_sided());

        let pbr = material.pbr_metallic_roughness();
        assert_eq!(pbr.metallic_factor(), MODEL_METALLIC);
        assert_eq!(pbr.roughness_factor(), MODEL_ROUGHNESS);
        assert_eq!(pbr.base_color_factor(), [1.0, 1.0, 1.0, 1.0]);
        assert!(pbr.base_color_texture().is_some());
    }

    #[test]
    fn a_translucent_model_blends_with_a_white_base_color_like_native() {
        let dir = fixture_dir();
        let path = dir.path().join("ghost.glb");
        let mut rsm = textured_rsm();
        rsm.alpha = 0.25;
        let build = build_model(&rsm, "hash").expect("build");

        write_model_glb(&path, &build, &textures()).expect("write glb");

        let (document, _, _) = gltf::import(&path).expect("reimport");
        let material = node_named(&document, "main")
            .mesh()
            .expect("mesh")
            .primitives()
            .next()
            .expect("primitive")
            .material();

        assert_eq!(material.alpha_mode(), gltf::material::AlphaMode::Blend);
        assert_eq!(material.alpha_cutoff(), None);
        assert!(material.double_sided());

        let pbr = material.pbr_metallic_roughness();
        assert_eq!(pbr.base_color_factor(), [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(pbr.metallic_factor(), MODEL_METALLIC);
        assert_eq!(pbr.roughness_factor(), MODEL_ROUGHNESS);
    }

    #[test]
    fn a_face_pointing_at_a_missing_texture_slot_gets_an_untextured_material() {
        let fixture = write_fixture();
        let (document, _, _) = gltf::import(&fixture.path).expect("reimport");

        let untextured = node_named(&document, "child")
            .mesh()
            .expect("child mesh")
            .primitives()
            .next()
            .expect("primitive")
            .material();

        assert!(
            untextured
                .pbr_metallic_roughness()
                .base_color_texture()
                .is_none()
        );
        assert_eq!(untextured.alpha_mode(), gltf::material::AlphaMode::Mask);
        assert_eq!(untextured.alpha_cutoff(), Some(MODEL_ALPHA_CUTOFF));
        assert!(untextured.double_sided());
    }

    #[test]
    fn keyframe_times_rescale_per_channel_against_that_channels_own_max_frame() {
        let dir = fixture_dir();
        let path = dir.path().join("windmill.glb");
        let rsm = animated_rsm();
        let build = build_model(&rsm, "hash").expect("build");

        write_model_glb(&path, &build, &textures()).expect("write glb");

        let (document, buffers, _) = gltf::import(&path).expect("reimport");
        let animations: Vec<_> = document.animations().collect();
        assert_eq!(animations.len(), 1);
        assert_eq!(animations[0].name(), Some(ANIMATION_NAME));

        let channels: Vec<_> = animations[0].channels().collect();
        assert_eq!(channels.len(), 2);

        let read = |channel: &gltf::animation::Channel| {
            let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
            reader.read_inputs().expect("inputs").collect::<Vec<f32>>()
        };

        // anim_len 4000 ms; positions top out at frame 8, rotations at frame 2.
        let translation = channels
            .iter()
            .find(|channel| channel.target().property() == gltf::animation::Property::Translation)
            .expect("translation channel");
        assert_eq!(translation.target().node().name(), Some("main"));
        assert_eq!(read(translation), vec![0.0, 2.0, 4.0]);

        let rotation = channels
            .iter()
            .find(|channel| channel.target().property() == gltf::animation::Property::Rotation)
            .expect("rotation channel");
        assert_eq!(rotation.target().node().name(), Some("child"));
        assert_eq!(read(rotation), vec![0.0, 4.0]);

        let reader = rotation.reader(|buffer| Some(&buffers[buffer.index()]));
        let gltf::animation::util::ReadOutputs::Rotations(rotations) =
            reader.read_outputs().expect("outputs")
        else {
            panic!("rotation channel must carry rotations");
        };
        assert_eq!(
            rotations.into_f32().collect::<Vec<[f32; 4]>>(),
            vec![[0.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 0.0]]
        );
    }

    #[test]
    fn a_model_without_keyframes_has_no_animation() {
        let fixture = write_fixture();
        let (document, _, _) = gltf::import(&fixture.path).expect("reimport");

        assert_eq!(document.animations().count(), 0);
    }

    #[test]
    fn keyframes_with_a_zero_anim_len_produce_no_animation() {
        let dir = fixture_dir();
        let path = dir.path().join("still.glb");
        let mut rsm = animated_rsm();
        rsm.anim_len = 0;
        let build = build_model(&rsm, "hash").expect("build");

        write_model_glb(&path, &build, &textures()).expect("write glb");

        let (document, _, _) = gltf::import(&path).expect("reimport");
        assert_eq!(document.animations().count(), 0);
    }

    #[test]
    fn the_root_carries_the_lif_model_stamp() {
        let fixture = write_fixture();
        let (document, _, _) = gltf::import(&fixture.path).expect("reimport");
        let root_json = document.into_json();

        let extensions = root_json.extensions.as_ref().expect("root extensions");
        let model: lif::LifModel =
            serde_json::from_value(extensions.others[lif::EXTENSION_MODEL].clone())
                .expect("LIF_model");

        assert_eq!(model.format_version, lif::FORMAT_VERSION);
        assert_eq!(model.rsm_hash, ModelFixture::RSM_HASH);
        assert_eq!(root_json.extensions_used, vec![lif::EXTENSION_MODEL]);
        assert!(root_json.extensions_required.is_empty());
    }

    #[test]
    fn the_phase_2_rsm1_glb_matches_its_pinned_digest() {
        let dir = fixture_dir();
        let rsm = animated_rsm();
        let build = build_model(&rsm, "hash").expect("build");
        let path = dir.path().join("phase2-rsm1.glb");
        write_model_glb(&path, &build, &textures()).expect("write");

        let digest = blake3::hash(&std::fs::read(path).expect("read"));

        assert_eq!(
            digest.to_hex().as_str(),
            "6b2cf1a22e709fdd4af7523dbff656a9183964c2acf7da0f32b26ee535301d6c"
        );
    }

    #[test]
    fn the_same_model_written_twice_is_byte_identical() {
        let dir = fixture_dir();
        let rsm = animated_rsm();
        let build = build_model(&rsm, "hash").expect("build");

        let again = dir.path().join("again");
        std::fs::create_dir(&again).expect("mkdir");

        let first = dir.path().join("windmill.glb");
        let second = again.join("windmill.glb");
        write_model_glb(&first, &build, &textures()).expect("write first");
        write_model_glb(&second, &build, &textures()).expect("write second");

        assert_eq!(
            std::fs::read(&first).expect("read"),
            std::fs::read(&second).expect("read")
        );
    }

    #[test]
    fn repeated_keyframe_numbers_fail_loudly() {
        let mut rsm = animated_rsm();
        rsm.nodes[0].pos_keyframes[2].frame = 4;
        let err = build_model(&rsm, "hash").expect_err("repeated keyframes must fail");

        assert!(
            err.to_string().contains("non-increasing translation"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn a_texture_count_mismatch_fails_loudly() {
        let dir = fixture_dir();
        let rsm = textured_rsm();
        let build = build_model(&rsm, "hash").expect("build");

        let err = write_model_glb(&dir.path().join("bad.glb"), &build, &[])
            .expect_err("missing textures must fail");

        assert!(
            err.to_string().contains("were exported"),
            "unexpected error: {err}"
        );
    }
}
