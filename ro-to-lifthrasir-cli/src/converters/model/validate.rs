//! Re-reads a written prop `.glb` and proves it still says what the
//! `ModelBuild` said. Mirrors `map/validate.rs`: nothing here trusts the
//! writer's in-memory state -- the file is parsed back from disk and every
//! value is compared against `ModelBuild`. Any mismatch is a loud error.

use crate::converters::gltf_out::ROOT_FIX;
use crate::converters::model::mesh::ModelBuild;
use anyhow::{Context, ensure};
use glam::{Quat, Vec3};
use lifthrasir_data::lif;
use std::fmt;
use std::path::Path;

/// Tolerance for values that survive a f32 encode/decode and a quaternion
/// round trip.
const EPSILON: f32 = 1e-4;

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

/// Re-read `glb_path` and assert it round-trips `build`.
///
/// `anim_len_ms` is the source RSM's `anim_len` -- the caller (Task 8's
/// orchestrator has the parsed `Rsm` in hand) supplies it so the animation
/// check can assert the keyframed channels' end time against the actual
/// expected duration, not just against each other.
///
/// Material validation checks only the flags that hold regardless of RSM
/// alpha -- `doubleSided`, `metallic 0`, `roughness 1` -- because the alpha
/// mode branch (`MASK` vs `BLEND`) depends on `rsm.alpha`, which `ModelBuild`
/// does not carry; that branch is covered by the writer's own tests instead.
pub fn validate(glb_path: &Path, build: &ModelBuild, anim_len_ms: i32) -> anyhow::Result<Counts> {
    let gltf::Gltf { document, blob } =
        gltf::Gltf::open(glb_path).with_context(|| format!("re-reading {}", glb_path.display()))?;
    let blob = blob.with_context(|| format!("{} has no BIN chunk", glb_path.display()))?;

    let counts = {
        let root = scene_root(&document)?;
        let resolved = validate_nodes(&root, &document, build)?;
        let (primitives, vertices) = validate_primitives(build, &resolved, &blob)?;
        let animation_channels = validate_animation(&document, &blob, build, anim_len_ms)?;
        validate_materials(&document)?;
        validate_root_fix(&root, build, &resolved, &blob)?;

        Counts {
            nodes: resolved.len(),
            primitives,
            vertices,
            animation_channels,
        }
    };
    validate_root_extensions(&document.into_json())?;

    Ok(counts)
}

fn scene_root(document: &gltf::Document) -> anyhow::Result<gltf::Node<'_>> {
    let scene = document
        .default_scene()
        .context("glb has no default scene")?;
    let roots: Vec<gltf::Node> = scene.nodes().collect();
    ensure!(
        roots.len() == 1,
        "glb scene must have exactly one root node, found {}",
        roots.len()
    );
    Ok(roots.into_iter().next().expect("checked length"))
}

/// Resolve each `ModelBuild` node to its glTF node by name, in build order,
/// checking the total node count and the parent/child wiring at once.
fn validate_nodes<'a>(
    root: &gltf::Node<'a>,
    document: &'a gltf::Document,
    build: &ModelBuild,
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
    }

    Ok(resolved)
}

/// Per-primitive vertex and index counts against `ModelBuild`; returns the
/// total primitive and vertex counts across the whole model.
fn validate_primitives(
    build: &ModelBuild,
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
            let positions = reader
                .read_positions()
                .with_context(|| format!("node '{}' primitive has no positions", node.name))?
                .count();
            ensure!(
                positions == expected.positions.len(),
                "node '{}' primitive has {positions} vertices, the build has {}",
                node.name,
                expected.positions.len()
            );

            let indices = reader
                .read_indices()
                .with_context(|| format!("node '{}' primitive has no indices", node.name))?
                .into_u32()
                .count();
            ensure!(
                indices == expected.indices.len(),
                "node '{}' primitive has {indices} indices, the build has {}",
                node.name,
                expected.indices.len()
            );

            primitives += 1;
            vertices += positions;
        }
    }

    Ok((primitives, vertices))
}

/// Channel count matches the nodes-with-keyframes count (translation and
/// rotation counted separately). Every channel with a non-degenerate frame
/// range must end at `anim_len_ms / 1000` seconds -- checking against the
/// actual source duration, not just cross-channel agreement, catches a
/// systematic rescale bug (e.g. a dropped `/ 1000.0`) that would otherwise
/// shift every channel by the same factor and still agree with itself. The
/// cross-channel check is kept alongside it: the per-channel rescale is
/// supposed to normalize every keyframed channel onto the one shared
/// duration, so channels disagreeing with each other is its own distinct
/// failure from either channel disagreeing with the source.
fn validate_animation(
    document: &gltf::Document,
    blob: &[u8],
    build: &ModelBuild,
    anim_len_ms: i32,
) -> anyhow::Result<usize> {
    let expected_channels: usize = build
        .nodes
        .iter()
        .map(|node| {
            usize::from(!node.pos_keyframes.is_empty())
                + usize::from(!node.rot_keyframes.is_empty())
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

    let expected_end = (anim_len_ms > 0).then(|| anim_len_ms as f32 / 1000.0);

    let mut durations = Vec::new();
    for channel in &channels {
        let reader = channel.reader(|_| Some(blob));
        let inputs: Vec<f32> = reader
            .read_inputs()
            .context("animation channel has no inputs")?
            .collect();
        let end = inputs.into_iter().fold(0.0_f32, f32::max);
        if end == 0.0 {
            continue;
        }

        if let Some(expected_end) = expected_end {
            ensure!(
                (end - expected_end).abs() < EPSILON,
                "animation channel ends at {end}, expected {expected_end} ({anim_len_ms} ms anim_len)"
            );
        }
        durations.push(end);
    }
    if let Some(first) = durations.first() {
        for end in &durations {
            ensure!(
                (end - first).abs() < EPSILON,
                "animation channels disagree on their end time: {first} vs {end}"
            );
        }
    }

    Ok(channels.len())
}

fn validate_materials(document: &gltf::Document) -> anyhow::Result<()> {
    for material in document.materials() {
        let name = material.name().unwrap_or("<unnamed>").to_string();
        ensure!(
            material.double_sided(),
            "material '{name}' is not double-sided"
        );

        let pbr = material.pbr_metallic_roughness();
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
/// the raw `ModelBuild` position, proving the synthetic root's pre-rotation
/// still cancels the runtime's `ROOT_FIX` exactly.
fn validate_root_fix(
    root: &gltf::Node,
    build: &ModelBuild,
    resolved: &[gltf::Node],
    blob: &[u8],
) -> anyhow::Result<()> {
    let (_, rotation, _) = root.transform().decomposed();
    let root_rotation = Quat::from_array(rotation);

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
    ensure!(
        (actual - expected).length() < EPSILON,
        "ROOT_FIX round trip on node '{}': expected {expected:?}, got {actual:?}",
        node.name
    );
    Ok(())
}

fn validate_root_extensions(root: &gltf_json::Root) -> anyhow::Result<()> {
    let value = root
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.others.get(lif::EXTENSION_MODEL))
        .context("glb has no LIF_model root extension")?;
    let model: lif::LifModel =
        serde_json::from_value(value.clone()).context("decoding LIF_model")?;
    ensure!(
        model.format_version == lif::FORMAT_VERSION,
        "LIF_model format_version is {}, this converter writes {}",
        model.format_version,
        lif::FORMAT_VERSION
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::model::fixtures::{animated_rsm, fixture_dir, textures, write_fixture};
    use crate::converters::model::mesh::build_model;
    use crate::converters::model::writer::write_model_glb;

    #[test]
    fn a_freshly_written_glb_validates_against_its_build() {
        let fixture = write_fixture();

        let counts = validate(&fixture.path, &fixture.build, fixture.rsm.anim_len)
            .expect("validation passes");

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
        let build = build_model(&rsm).expect("build");
        write_model_glb(&path, &rsm, "hash", &build, &textures()).expect("write glb");

        let counts = validate(&path, &build, rsm.anim_len).expect("validation passes");

        assert_eq!(counts.animation_channels, 2);
    }

    fn assert_fails(fixture: &crate::converters::model::fixtures::ModelFixture, expected: &str) {
        let err = validate(&fixture.path, &fixture.build, fixture.rsm.anim_len)
            .expect_err("validation must fail");
        let message = format!("{err:#}");
        assert!(
            message.contains(expected),
            "expected an error mentioning '{expected}', got: {message}"
        );
    }

    #[test]
    fn a_renamed_node_fails() {
        let mut fixture = write_fixture();
        fixture.build.nodes[1].name = "renamed".to_string();

        assert_fails(&fixture, "no node named 'renamed'");
    }

    #[test]
    fn a_dropped_primitive_fails() {
        let mut fixture = write_fixture();
        fixture.build.nodes[0].primitives.pop();

        assert_fails(&fixture, "primitive(s)");
    }

    #[test]
    fn a_truncated_primitive_fails() {
        let mut fixture = write_fixture();
        fixture.build.nodes[0].primitives[0]
            .positions
            .push([0.0, 0.0, 0.0]);

        assert_fails(&fixture, "vertices");
    }

    #[test]
    fn a_reparented_node_fails() {
        let mut fixture = write_fixture();
        fixture.build.nodes[1].parent = None;

        assert_fails(&fixture, "is not a child of");
    }

    #[test]
    fn a_duration_that_disagrees_with_the_source_anim_len_fails() {
        let dir = fixture_dir();
        let path = dir.path().join("windmill.glb");
        let rsm = animated_rsm();
        let build = build_model(&rsm).expect("build");
        write_model_glb(&path, &rsm, "hash", &build, &textures()).expect("write glb");

        let err =
            validate(&path, &build, rsm.anim_len * 2).expect_err("mismatched duration must fail");

        let message = format!("{err:#}");
        assert!(
            message.contains("ends at"),
            "expected an error mentioning 'ends at', got: {message}"
        );
    }

    #[test]
    fn a_shifted_vertex_position_fails_the_root_fix_round_trip() {
        let mut fixture = write_fixture();
        fixture.build.nodes[0].primitives[0].positions[0][0] += 5.0;

        assert_fails(&fixture, "ROOT_FIX round trip");
    }
}
