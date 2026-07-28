//! Re-reads a written prop `.glb` and proves it still says what the
//! normalized model said. Mirrors `map/validate.rs`: nothing here trusts the
//! writer's in-memory state -- the file is parsed back from disk and every
//! value is compared against the shared contract. Any mismatch is a loud error.

use crate::converters::gltf_out::{EPSILON, ROOT_FIX, ensure_close, root_extension, scene_root};
use crate::converters::map::textures::TextureOut;
use crate::converters::model::normalized::{AlphaMode, NormalizedModel};
use anyhow::{Context, bail, ensure};
use glam::{Quat, Vec3};
use lifthrasir_data::lif;
use std::fmt;
use std::path::Path;

/// What the validated glb contains; printed as the conversion summary.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub nodes: usize,
    pub primitives: usize,
    pub vertices: usize,
    pub animation_channels: usize,
}

impl fmt::Display for Counts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} nodes, {} primitives, {} vertices, {} animation channels",
            self.nodes, self.primitives, self.vertices, self.animation_channels
        )
    }
}

pub(super) fn validate_contract(model: &NormalizedModel) -> anyhow::Result<()> {
    ensure!(
        model.duration_ms.is_finite() && model.duration_ms >= 0.0,
        "invalid model duration"
    );
    ensure!(
        !model.provenance.source_version.is_empty(),
        "missing source version provenance"
    );
    ensure!(
        !model.provenance.source_hash.is_empty(),
        "missing source hash provenance"
    );

    let actual_roots: Vec<usize> = model
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| node.parent.is_none().then_some(index))
        .collect();
    let mut declared_roots = model.roots.clone();
    declared_roots.sort_unstable();
    ensure!(
        declared_roots == actual_roots,
        "normalized roots do not match parent links"
    );
    ensure!(
        model.nodes.iter().any(|node| !node.primitives.is_empty()),
        "normalized model has no geometry"
    );

    let mut node_names = std::collections::HashSet::new();
    for (index, node) in model.nodes.iter().enumerate() {
        ensure!(!node.name.is_empty(), "node {index} has an empty name");
        ensure!(
            node_names.insert(node.name.as_str()),
            "duplicate normalized node name '{}'",
            node.name
        );
        ensure!(
            node.translation.iter().all(|value| value.is_finite())
                && node.rotation.iter().all(|value| value.is_finite())
                && node.scale.iter().all(|value| value.is_finite()),
            "node '{}' has non-finite base TRS",
            node.name
        );
        if let Some(parent) = node.parent {
            ensure!(
                parent < model.nodes.len(),
                "node '{}' has invalid parent {parent}",
                node.name
            );
        }
        let mut ancestor = index;
        for depth in 0..=model.nodes.len() {
            let Some(parent) = model.nodes[ancestor].parent else {
                ensure!(
                    model.roots.contains(&ancestor),
                    "node '{}' does not reach a declared root",
                    node.name
                );
                break;
            };
            ensure!(
                depth < model.nodes.len(),
                "node '{}' belongs to a parent cycle",
                node.name
            );
            ensure!(
                parent < model.nodes.len(),
                "node '{}' reaches invalid parent {parent}",
                node.name
            );
            ancestor = parent;
        }
        for primitive in &node.primitives {
            ensure!(
                primitive.material < model.materials.len(),
                "node '{}' has invalid material {}",
                node.name,
                primitive.material
            );
            let vertex_count = primitive.positions.len();
            ensure!(
                vertex_count > 0,
                "node '{}' has an empty primitive",
                node.name
            );
            ensure!(
                vertex_count == primitive.normals.len()
                    && vertex_count == primitive.uv0.len()
                    && primitive
                        .uv1
                        .as_ref()
                        .is_none_or(|uv1| uv1.len() == vertex_count),
                "node '{}' primitive attribute counts disagree",
                node.name
            );
            ensure!(
                primitive
                    .positions
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite())
                    && primitive
                        .normals
                        .iter()
                        .flatten()
                        .all(|value| value.is_finite())
                    && primitive
                        .uv0
                        .iter()
                        .flatten()
                        .all(|value| value.is_finite())
                    && primitive
                        .uv1
                        .as_ref()
                        .is_none_or(|uv1| { uv1.iter().flatten().all(|value| value.is_finite()) }),
                "node '{}' primitive has non-finite attributes",
                node.name
            );
            ensure!(
                !primitive.indices.is_empty()
                    && primitive.indices.len() % 3 == 0
                    && primitive
                        .indices
                        .iter()
                        .all(|index| (*index as usize) < vertex_count),
                "node '{}' primitive has invalid triangle indices",
                node.name
            );
        }
        ensure!(
            node.translation_track
                .keys
                .iter()
                .flat_map(|key| key.value)
                .all(|value| value.is_finite())
                && node
                    .rotation_track
                    .keys
                    .iter()
                    .flat_map(|key| key.value)
                    .all(|value| value.is_finite())
                && node
                    .scale_track
                    .keys
                    .iter()
                    .flat_map(|key| key.value)
                    .all(|value| value.is_finite()),
            "node '{}' has non-finite track values",
            node.name
        );
        for (property, times) in [
            (
                "translation",
                node.translation_track
                    .keys
                    .iter()
                    .map(|key| key.time_ms)
                    .collect::<Vec<_>>(),
            ),
            (
                "rotation",
                node.rotation_track
                    .keys
                    .iter()
                    .map(|key| key.time_ms)
                    .collect(),
            ),
            (
                "scale",
                node.scale_track
                    .keys
                    .iter()
                    .map(|key| key.time_ms)
                    .collect(),
            ),
        ] {
            ensure!(
                times
                    .iter()
                    .all(|time| time.is_finite() && *time >= 0.0 && *time <= model.duration_ms)
                    && !times.windows(2).any(|pair| pair[1] <= pair[0])
                    && times
                        .last()
                        .is_none_or(|last| *last == model.duration_ms && model.duration_ms > 0.0),
                "node '{}' has invalid {property} track times",
                node.name
            );
        }
    }
    for material in &model.materials {
        if let Some(texture) = material.texture {
            ensure!(
                texture < model.textures.len(),
                "material has invalid texture {texture}"
            );
        }
    }
    Ok(())
}

/// Re-read `glb_path` and assert it round-trips `build`.
///
/// The normalized duration, roots, geometry, materials, TRS tracks, and
/// provenance are all checked against the reimported file.
pub fn validate(
    glb_path: &Path,
    build: &NormalizedModel,
    textures: &[TextureOut],
) -> anyhow::Result<Counts> {
    validate_contract(build)?;
    let gltf::Gltf { document, blob } =
        gltf::Gltf::open(glb_path).with_context(|| format!("re-reading {}", glb_path.display()))?;
    let blob = blob.with_context(|| format!("{} has no BIN chunk", glb_path.display()))?;

    let counts = {
        let root = scene_root(&document)?;
        let resolved = validate_nodes(&root, &document, build)?;
        let (primitives, vertices) = validate_primitives(build, &resolved, &blob)?;
        let animation_channels = validate_animation(&document, &blob, build, &resolved)?;
        validate_materials(&document, build, textures)?;
        validate_root_fix(&root, build, &resolved, &blob)?;

        Counts {
            nodes: resolved.len(),
            primitives,
            vertices,
            animation_channels,
        }
    };
    validate_root_extensions(&document.into_json(), build)?;

    Ok(counts)
}

/// Resolve each `NormalizedModel` node to its glTF node by name, in build order,
/// checking the total node count and the parent/child wiring at once.
fn validate_nodes<'a>(
    root: &gltf::Node<'a>,
    document: &'a gltf::Document,
    build: &NormalizedModel,
) -> anyhow::Result<Vec<gltf::Node<'a>>> {
    let total = document.nodes().count();
    ensure!(
        total == build.nodes.len() + 1,
        "glb has {total} node(s), expected {} ({} model node(s) plus the synthetic root)",
        build.nodes.len() + 1,
        build.nodes.len()
    );

    let resolved: Vec<gltf::Node<'a>> = build
        .nodes
        .iter()
        .map(|node| {
            document
                .nodes()
                .find(|candidate| candidate.name() == Some(node.name.as_str()))
                .with_context(|| format!("glb has no node named '{}'", node.name))
        })
        .collect::<anyhow::Result<_>>()?;

    let root_children: Vec<usize> = root.children().map(|node| node.index()).collect();
    let expected_roots: Vec<usize> = build
        .roots
        .iter()
        .map(|index| resolved[*index].index())
        .collect();
    ensure!(
        root_children == expected_roots,
        "glb roots {root_children:?} do not match normalized roots {expected_roots:?}"
    );

    for (index, node) in build.nodes.iter().enumerate() {
        let expected_parent = match node.parent {
            Some(parent_index) => &resolved[parent_index],
            None => root,
        };
        let is_child = expected_parent
            .children()
            .any(|child| child.index() == resolved[index].index());
        ensure!(
            is_child,
            "node '{}' is not a child of '{}' in the glb",
            node.name,
            expected_parent.name().unwrap_or("<unnamed>")
        );
        let (translation, rotation, scale) = resolved[index].transform().decomposed();
        ensure!(
            translation == node.translation && rotation == node.rotation && scale == node.scale,
            "node '{}' local TRS disagrees with normalized data",
            node.name
        );
    }

    Ok(resolved)
}

/// Per-primitive vertex and index counts against `NormalizedModel`; returns the
/// total primitive and vertex counts across the whole model.
fn validate_primitives(
    build: &NormalizedModel,
    resolved: &[gltf::Node],
    blob: &[u8],
) -> anyhow::Result<(usize, usize)> {
    let mut primitives = 0;
    let mut vertices = 0;

    for (node, glb_node) in build.nodes.iter().zip(resolved) {
        if node.primitives.is_empty() {
            continue;
        }
        let mesh = glb_node
            .mesh()
            .with_context(|| format!("node '{}' carries no mesh", node.name))?;
        let glb_primitives: Vec<gltf::Primitive> = mesh.primitives().collect();
        ensure!(
            glb_primitives.len() == node.primitives.len(),
            "node '{}' has {} primitive(s), the build has {}",
            node.name,
            glb_primitives.len(),
            node.primitives.len()
        );

        for (expected, primitive) in node.primitives.iter().zip(&glb_primitives) {
            let reader = primitive.reader(|_| Some(blob));
            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .with_context(|| format!("node '{}' primitive has no positions", node.name))?
                .collect();
            ensure!(
                positions == expected.positions,
                "node '{}' primitive positions disagree with normalized data",
                node.name
            );
            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .with_context(|| format!("node '{}' primitive has no normals", node.name))?
                .collect();
            ensure!(
                normals == expected.normals,
                "node '{}' primitive normals disagree with normalized data",
                node.name
            );
            let uv0: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .with_context(|| format!("node '{}' primitive has no UV0", node.name))?
                .into_f32()
                .collect();
            ensure!(
                uv0 == expected.uv0,
                "node '{}' primitive UV0 disagrees with normalized data",
                node.name
            );
            let indices: Vec<u32> = reader
                .read_indices()
                .with_context(|| format!("node '{}' primitive has no indices", node.name))?
                .into_u32()
                .collect();
            ensure!(
                indices == expected.indices,
                "node '{}' primitive indices disagree with normalized data",
                node.name
            );
            ensure!(
                primitive.material().index() == Some(expected.material),
                "node '{}' primitive material disagrees with normalized data",
                node.name
            );

            primitives += 1;
            vertices += positions.len();
        }
    }

    Ok((primitives, vertices))
}

/// Channel targets, strict millisecond times, values, and model-duration
/// coverage must all round-trip through standard glTF animation.
fn validate_animation(
    document: &gltf::Document,
    blob: &[u8],
    build: &NormalizedModel,
    resolved: &[gltf::Node],
) -> anyhow::Result<usize> {
    let expected_channels: usize = build
        .nodes
        .iter()
        .map(|node| {
            usize::from(!node.translation_track.keys.is_empty())
                + usize::from(!node.rotation_track.keys.is_empty())
                + usize::from(!node.scale_track.keys.is_empty())
        })
        .sum();

    let animations: Vec<gltf::Animation> = document.animations().collect();
    if expected_channels == 0 {
        ensure!(
            animations.is_empty(),
            "glb has an animation but the build carries no keyframes"
        );
        return Ok(0);
    }

    ensure!(
        animations.len() == 1,
        "glb has {} animation(s), expected 1",
        animations.len()
    );
    let channels: Vec<gltf::animation::Channel> = animations[0].channels().collect();
    ensure!(
        channels.len() == expected_channels,
        "glb animation has {} channel(s), the build has {expected_channels} keyframed channel(s)",
        channels.len()
    );

    let mut expected = Vec::new();
    for (index, node) in build.nodes.iter().enumerate() {
        if !node.translation_track.keys.is_empty() {
            expected.push((
                resolved[index].index(),
                gltf::animation::Property::Translation,
                node.translation_track
                    .keys
                    .iter()
                    .map(|key| key.time_ms / 1000.0)
                    .collect::<Vec<_>>(),
                node.translation_track
                    .keys
                    .iter()
                    .flat_map(|key| key.value)
                    .collect::<Vec<_>>(),
            ));
        }
        if !node.rotation_track.keys.is_empty() {
            expected.push((
                resolved[index].index(),
                gltf::animation::Property::Rotation,
                node.rotation_track
                    .keys
                    .iter()
                    .map(|key| key.time_ms / 1000.0)
                    .collect(),
                node.rotation_track
                    .keys
                    .iter()
                    .flat_map(|key| key.value)
                    .collect(),
            ));
        }
        if !node.scale_track.keys.is_empty() {
            expected.push((
                resolved[index].index(),
                gltf::animation::Property::Scale,
                node.scale_track
                    .keys
                    .iter()
                    .map(|key| key.time_ms / 1000.0)
                    .collect(),
                node.scale_track
                    .keys
                    .iter()
                    .flat_map(|key| key.value)
                    .collect(),
            ));
        }
    }

    for (channel, (node, property, times, values)) in channels.iter().zip(expected) {
        ensure!(
            channel.target().node().index() == node,
            "animation channel targets the wrong node"
        );
        ensure!(
            channel.target().property() == property,
            "animation channel targets the wrong property"
        );
        let reader = channel.reader(|_| Some(blob));
        let actual_times: Vec<f32> = reader
            .read_inputs()
            .context("animation channel has no inputs")?
            .collect();
        ensure!(
            actual_times == times,
            "animation channel times disagree with normalized data"
        );
        let actual_values: Vec<f32> = match reader
            .read_outputs()
            .context("animation channel has no outputs")?
        {
            gltf::animation::util::ReadOutputs::Translations(values) => values.flatten().collect(),
            gltf::animation::util::ReadOutputs::Rotations(values) => {
                values.into_f32().flatten().collect()
            }
            gltf::animation::util::ReadOutputs::Scales(values) => values.flatten().collect(),
            _ => bail!("unsupported animation output"),
        };
        ensure!(
            actual_values == values,
            "animation channel values disagree with normalized data"
        );
        if times.iter().any(|time| *time > 0.0) {
            let end = *times
                .last()
                .context("animation channel has no final time")?;
            let expected_end = build.duration_ms / 1000.0;
            ensure!(
                (end - expected_end).abs() < EPSILON,
                "animation channel ends at {end}, expected {expected_end} ({} ms duration)",
                build.duration_ms
            );
        }
    }

    Ok(channels.len())
}

fn validate_materials(
    document: &gltf::Document,
    model: &NormalizedModel,
    textures: &[TextureOut],
) -> anyhow::Result<()> {
    ensure!(
        document.materials().count() == model.materials.len(),
        "glb material count disagrees with normalized data"
    );
    for (material, expected) in document.materials().zip(&model.materials) {
        let name = material.name().unwrap_or("<unnamed>").to_string();
        ensure!(
            material.double_sided() == expected.two_sided,
            "material '{name}' two-sidedness disagrees with normalized data"
        );
        match expected.alpha {
            AlphaMode::Mask { cutoff } => {
                ensure!(
                    material.alpha_mode() == gltf::material::AlphaMode::Mask,
                    "material '{name}' is not masked"
                );
                ensure!(
                    material.alpha_cutoff() == Some(cutoff),
                    "material '{name}' alpha cutoff disagrees"
                );
            }
            AlphaMode::Blend => ensure!(
                material.alpha_mode() == gltf::material::AlphaMode::Blend,
                "material '{name}' is not blended"
            ),
        }
        let pbr = material.pbr_metallic_roughness();
        let expected_texture = expected
            .texture
            .map(|index| {
                textures.get(index).with_context(|| {
                    format!("material '{name}' references missing exported texture {index}")
                })
            })
            .transpose()?;
        match (pbr.base_color_texture(), expected_texture) {
            (None, None) => ensure!(
                material.name().is_none(),
                "untextured material '{name}' unexpectedly has a name"
            ),
            (Some(info), Some(expected_texture)) => {
                ensure!(
                    material.name() == Some(expected_texture.source_name.as_str()),
                    "material '{name}' source name disagrees with exported texture"
                );
                match info.texture().source().source() {
                    gltf::image::Source::Uri { uri, .. } => ensure!(
                        uri == expected_texture.relative_path,
                        "material '{name}' URI '{uri}' differs from '{}'",
                        expected_texture.relative_path
                    ),
                    gltf::image::Source::View { .. } => {
                        bail!("material '{name}' unexpectedly embeds its texture")
                    }
                }
            }
            _ => bail!("material '{name}' texture presence disagrees with normalized data"),
        }
        ensure!(
            pbr.metallic_factor() == 0.0,
            "material '{name}' metallic_factor is {}, expected 0.0",
            pbr.metallic_factor()
        );
        ensure!(
            pbr.roughness_factor() == 1.0,
            "material '{name}' roughness_factor is {}, expected 1.0",
            pbr.roughness_factor()
        );
    }
    Ok(())
}

/// `ROOT_FIX` round trip on the first vertex of the first node that carries
/// geometry: `ROOT_FIX * (root_rotation * imported_vertex)` must reproduce
/// the raw `NormalizedModel` position, proving the synthetic root's pre-rotation
/// still cancels the runtime's `ROOT_FIX` exactly.
fn validate_root_fix(
    root: &gltf::Node,
    build: &NormalizedModel,
    resolved: &[gltf::Node],
    blob: &[u8],
) -> anyhow::Result<()> {
    let (translation, rotation, scale) = root.transform().decomposed();
    ensure_close(
        "synthetic root translation",
        Vec3::from_array(translation),
        Vec3::ZERO,
    )?;
    ensure_close("synthetic root scale", Vec3::from_array(scale), Vec3::ONE)?;
    let root_rotation = Quat::from_array(rotation);
    ensure!(
        root_rotation.dot(ROOT_FIX).abs() >= 1.0 - EPSILON,
        "synthetic root rotation does not undo ROOT_FIX: {root_rotation:?}"
    );

    let (node, glb_node) = build
        .nodes
        .iter()
        .zip(resolved)
        .find(|(node, _)| !node.primitives.is_empty())
        .context("no node in the build carries geometry to check the ROOT_FIX round trip")?;
    let expected = Vec3::from(node.primitives[0].positions[0]);

    let mesh = glb_node
        .mesh()
        .with_context(|| format!("node '{}' carries no mesh", node.name))?;
    let primitive = mesh
        .primitives()
        .next()
        .with_context(|| format!("node '{}' mesh has no primitives", node.name))?;
    let reader = primitive.reader(|_| Some(blob));
    let imported = reader
        .read_positions()
        .with_context(|| format!("node '{}' primitive has no positions", node.name))?
        .next()
        .with_context(|| format!("node '{}' primitive has no vertices", node.name))?;

    let actual = ROOT_FIX * (root_rotation * Vec3::from(imported));
    ensure_close(
        &format!("ROOT_FIX round trip on node '{}'", node.name),
        actual,
        expected,
    )
}

fn validate_root_extensions(
    root: &gltf_json::Root,
    expected: &NormalizedModel,
) -> anyhow::Result<()> {
    let model: lif::LifModel = root_extension(root, lif::EXTENSION_MODEL)?;
    ensure!(
        model.format_version == lif::FORMAT_VERSION,
        "LIF_model format_version is {}, this converter writes {}",
        model.format_version,
        lif::FORMAT_VERSION
    );
    ensure!(
        model.rsm_hash == expected.provenance.source_hash,
        "LIF_model source hash disagrees with normalized provenance"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::model::fixtures::{animated_rsm, fixture_dir, textures, write_fixture};
    use crate::converters::model::mesh::build_model;
    use crate::converters::model::normalized::{NormalizedKey, NormalizedTrack};
    use crate::converters::model::writer::write_model_glb;

    #[test]
    fn a_freshly_written_glb_validates_against_its_build() {
        let fixture = write_fixture();

        let counts =
            validate(&fixture.path, &fixture.model, &textures()).expect("validation passes");

        assert_eq!(
            counts,
            Counts {
                nodes: 2,
                primitives: 3,
                vertices: 9,
                animation_channels: 0,
            }
        );
        assert_eq!(
            counts.to_string(),
            "2 nodes, 3 primitives, 9 vertices, 0 animation channels"
        );
    }

    #[test]
    fn an_animated_glb_reports_its_channel_count() {
        let dir = fixture_dir();
        let path = dir.path().join("windmill.glb");
        let rsm = animated_rsm();
        let build = build_model(&rsm, "hash").expect("build");
        write_model_glb(&path, &build, &textures()).expect("write glb");

        let counts = validate(&path, &build, &textures()).expect("validation passes");

        assert_eq!(counts.animation_channels, 2);
    }

    fn assert_fails(fixture: &crate::converters::model::fixtures::ModelFixture, expected: &str) {
        let err =
            validate(&fixture.path, &fixture.model, &textures()).expect_err("validation must fail");
        let message = format!("{err:#}");
        assert!(
            message.contains(expected),
            "expected an error mentioning '{expected}', got: {message}"
        );
    }

    #[test]
    fn a_renamed_node_fails() {
        let mut fixture = write_fixture();
        fixture.model.nodes[1].name = "renamed".to_string();

        assert_fails(&fixture, "no node named 'renamed'");
    }

    #[test]
    fn a_dropped_primitive_fails() {
        let mut fixture = write_fixture();
        fixture.model.nodes[0].primitives.pop();

        assert_fails(&fixture, "primitive(s)");
    }

    #[test]
    fn a_truncated_primitive_fails() {
        let mut fixture = write_fixture();
        fixture.model.nodes[0].primitives[0]
            .positions
            .push([0.0, 0.0, 0.0]);

        assert_fails(&fixture, "attribute counts disagree");
    }

    #[test]
    fn a_reparented_node_fails() {
        let mut fixture = write_fixture();
        fixture.model.nodes[1].parent = None;

        assert_fails(&fixture, "normalized roots");
    }

    #[test]
    fn a_duration_that_disagrees_with_the_source_anim_len_fails() {
        let dir = fixture_dir();
        let path = dir.path().join("windmill.glb");
        let rsm = animated_rsm();
        let build = build_model(&rsm, "hash").expect("build");
        write_model_glb(&path, &build, &textures()).expect("write glb");
        let mut expected = build.clone();
        expected.duration_ms *= 2.0;

        let err =
            validate(&path, &expected, &textures()).expect_err("mismatched duration must fail");

        let message = format!("{err:#}");
        assert!(
            message.contains("invalid translation track times"),
            "expected a translation-duration error, got: {message}"
        );
    }

    #[test]
    fn duplicate_names_and_invalid_indices_fail_contract_validation() {
        let mut fixture = write_fixture();
        fixture.model.nodes[1].name = fixture.model.nodes[0].name.clone();
        assert_fails(&fixture, "duplicate normalized node name");

        let mut fixture = write_fixture();
        let vertex_count = fixture.model.nodes[0].primitives[0].positions.len() as u32;
        fixture.model.nodes[0].primitives[0].indices[0] = vertex_count;
        assert_fails(&fixture, "invalid triangle indices");

        let mut fixture = write_fixture();
        for node in &mut fixture.model.nodes {
            node.primitives.clear();
        }
        assert_fails(&fixture, "normalized model has no geometry");
    }

    #[test]
    fn one_key_track_and_swapped_material_texture_fail_validation() {
        let mut fixture = write_fixture();
        fixture.model.duration_ms = 1000.0;
        fixture.model.nodes[0].scale_track.keys = vec![NormalizedKey {
            time_ms: 0.0,
            value: [1.0; 3],
        }];
        assert_fails(&fixture, "invalid scale track times");

        let mut fixture = write_fixture();
        let material = fixture
            .model
            .materials
            .iter_mut()
            .find(|material| material.texture == Some(0))
            .unwrap();
        material.texture = Some(1);
        assert_fails(&fixture, "source name disagrees");
    }

    #[test]
    fn a_shifted_vertex_position_fails_the_root_fix_round_trip() {
        let mut fixture = write_fixture();
        fixture.model.nodes[0].primitives[0].positions[0][0] += 5.0;

        assert_fails(&fixture, "positions disagree");
    }

    #[test]
    fn normalized_scale_animation_round_trips_strict_times_and_duration() {
        let fixture = write_fixture();
        let path = fixture._dir.path().join("scale.glb");
        let mut model = fixture.model.clone();
        model.duration_ms = 1250.0;
        model.nodes[0].scale_track = NormalizedTrack {
            keys: vec![
                NormalizedKey {
                    time_ms: 0.0,
                    value: [1.0, 1.0, 1.0],
                },
                NormalizedKey {
                    time_ms: 625.0,
                    value: [2.0, 3.0, 4.0],
                },
                NormalizedKey {
                    time_ms: 1250.0,
                    value: [4.0, 5.0, 6.0],
                },
            ],
        };

        write_model_glb(&path, &model, &textures()).expect("write scale animation");
        let counts = validate(&path, &model, &textures()).expect("validate scale animation");
        assert_eq!(counts.animation_channels, 1);

        let (document, buffers, _) = gltf::import(path).expect("reimport");
        let channel = document
            .animations()
            .next()
            .expect("animation")
            .channels()
            .next()
            .expect("scale channel");
        assert_eq!(
            channel.target().property(),
            gltf::animation::Property::Scale
        );
        let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
        assert_eq!(
            reader.read_inputs().expect("times").collect::<Vec<_>>(),
            vec![0.0, 0.625, 1.25]
        );
        let gltf::animation::util::ReadOutputs::Scales(values) =
            reader.read_outputs().expect("scales")
        else {
            panic!("expected scale outputs");
        };
        assert_eq!(
            values.collect::<Vec<_>>(),
            vec![[1.0, 1.0, 1.0], [2.0, 3.0, 4.0], [4.0, 5.0, 6.0]]
        );
    }
}
