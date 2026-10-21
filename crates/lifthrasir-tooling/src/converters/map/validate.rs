//! Re-reads a written `.glb` and proves it still says what the document said.
//!
//! Nothing here trusts the writer's in-memory state: the file is parsed back
//! from disk and every value is compared against the [`MapDocument`] it was
//! written from. Any mismatch is an error -- a truncated or silently wrong map
//! must never reach the pak.

use crate::converters::gltf_out::ROOT_FIX;
use crate::converters::gltf_out::{ensure_close, root_extension, scene_root};
use crate::converters::map::document::{MapDocument, MapLight, MapNode, MapWater};
use crate::converters::map::terrain::TerrainPrimitive;
use anyhow::{Context, bail, ensure};
use glam::{Quat, Vec3};
use lifthrasir_data::lif;
use std::fmt;
use std::path::Path;

/// What the validated glb contains; printed as the conversion summary so a
/// truncated map is visible at a glance.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub meshes: usize,
    pub lights: usize,
    pub sounds: usize,
    pub effects: usize,
    pub props: usize,
}

impl fmt::Display for Counts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} meshes, {} lights, {} sounds, {} effects, {} props",
            self.meshes, self.lights, self.sounds, self.effects, self.props
        )
    }
}

/// Re-read `glb_path` and assert it round-trips `doc`.
pub fn validate(glb_path: &Path, doc: &MapDocument) -> anyhow::Result<Counts> {
    doc.validate()
        .map_err(|errors| anyhow::anyhow!("invalid map document: {}", errors.join("; ")))?;

    let gltf::Gltf { document, blob } =
        gltf::Gltf::open(glb_path).with_context(|| format!("re-reading {}", glb_path.display()))?;
    let blob = blob.with_context(|| format!("{} has no BIN chunk", glb_path.display()))?;

    let counts = {
        let root = scene_root(&document)?;
        validate_nodes(&root, &blob, doc)?
    };
    validate_root_extensions(&document.into_json(), &blob, doc)?;

    Ok(counts)
}

/// The scene root's children are, in order: the terrain node (when the map has
/// geometry), then one node per light, prop, sound and effect. Walking them in
/// that order validates placement and ordering at once.
fn validate_nodes(root: &gltf::Node, blob: &[u8], doc: &MapDocument) -> anyhow::Result<Counts> {
    let children: Vec<gltf::Node> = root.children().collect();
    let mut next = 0;

    let primitives: Vec<&TerrainPrimitive> = doc
        .terrain
        .iter()
        .filter(|primitive| !primitive.positions.is_empty())
        .collect();

    let meshes = if primitives.is_empty() {
        0
    } else {
        let node = children.first().context("glb has no terrain node")?;
        next += 1;
        validate_terrain(node, blob, &primitives)?
    };

    let emitted = &children[next..];
    let expected = doc.lights.len() + doc.props.len() + doc.sounds.len() + doc.effects.len();
    ensure!(
        emitted.len() == expected,
        "glb has {} object nodes, the document emits {expected}",
        emitted.len()
    );

    let mut counts = Counts {
        meshes,
        ..Default::default()
    };
    let mut nodes = emitted.iter();

    for light in &doc.lights {
        let node = nodes.next().context("glb is missing a light node")?;
        validate_light(node, light)?;
        counts.lights += 1;
    }
    for prop in &doc.props {
        let node = nodes.next().context("glb is missing a prop node")?;
        validate_prop(node, prop)?;
        counts.props += 1;
    }
    for sound in &doc.sounds {
        let node = nodes.next().context("glb is missing a sound node")?;
        validate_emitter(node, lif::EXTRAS_AUDIO, sound)?;
        counts.sounds += 1;
    }
    for effect in &doc.effects {
        let node = nodes.next().context("glb is missing an effect node")?;
        validate_emitter(node, lif::EXTRAS_EFFECT, effect)?;
        counts.effects += 1;
    }

    Ok(counts)
}

fn validate_terrain(
    node: &gltf::Node,
    blob: &[u8],
    expected: &[&TerrainPrimitive],
) -> anyhow::Result<usize> {
    let mesh = node.mesh().context("terrain node carries no mesh")?;
    let primitives: Vec<gltf::Primitive> = mesh.primitives().collect();
    ensure!(
        primitives.len() == expected.len(),
        "glb terrain has {} primitives, the document builds {}",
        primitives.len(),
        expected.len()
    );

    for (primitive, expected) in primitives.iter().zip(expected) {
        let reader = primitive.reader(|_| Some(blob));
        let positions = reader
            .read_positions()
            .with_context(|| format!("primitive '{}' has no positions", expected.texture))?
            .count();
        ensure!(
            positions == expected.positions.len(),
            "primitive '{}' has {positions} vertices, the document builds {}",
            expected.texture,
            expected.positions.len()
        );

        let indices = reader
            .read_indices()
            .with_context(|| format!("primitive '{}' has no indices", expected.texture))?
            .into_u32()
            .count();
        ensure!(
            indices == expected.indices.len(),
            "primitive '{}' has {indices} indices, the document builds {}",
            expected.texture,
            expected.indices.len()
        );
    }

    Ok(primitives.len())
}

fn validate_light(node: &gltf::Node, light: &MapLight) -> anyhow::Result<()> {
    let punctual = node.light().context("light node carries no KHR light")?;
    match light {
        MapLight::Sun {
            rotation,
            color,
            lux,
        } => {
            ensure!(
                matches!(
                    punctual.kind(),
                    gltf::khr_lights_punctual::Kind::Directional
                ),
                "sun node is not a directional light"
            );
            ensure!(
                punctual.color() == *color,
                "sun color {:?} differs from the document {:?}",
                punctual.color(),
                color
            );
            ensure!(
                punctual.intensity() == *lux,
                "sun intensity {} differs from the document {lux}",
                punctual.intensity()
            );

            let (_, actual, _) = node.transform().decomposed();
            let world_direction = (ROOT_FIX * Quat::from_array(actual)) * Vec3::NEG_Z;
            // The glb rotation already carries the fix, so applying the fix
            // again recovers the native rotation.
            let expected = *rotation * Vec3::NEG_Z;
            ensure_close("sun direction", world_direction, expected)
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
            ensure!(
                matches!(punctual.kind(), gltf::khr_lights_punctual::Kind::Spot {
                    inner_cone_angle,
                    outer_cone_angle,
                } if inner_cone_angle == *inner_angle && outer_cone_angle == *outer_angle),
                "spot light node is not a spot light with the document's cone angles"
            );
            ensure!(
                punctual.color() == *color,
                "spot light color {:?} differs from the document {:?}",
                punctual.color(),
                color
            );
            ensure!(
                punctual.intensity() == *candela,
                "spot light intensity {} differs from the document {candela}",
                punctual.intensity()
            );
            ensure!(
                punctual.range() == Some(*range),
                "spot light range {:?} differs from the document {range}",
                punctual.range()
            );

            let (actual, actual_rotation, _) = node.transform().decomposed();
            ensure_close(
                "spot light position",
                ROOT_FIX * Vec3::from(actual),
                *translation,
            )?;

            // The glb rotation carries the fix; applying it again recovers the
            // native rotation, as with the sun.
            let native = ROOT_FIX * Quat::from_array(actual_rotation);
            let wanted = *rotation;
            for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                ensure_close("spot light rotation", native * axis, wanted * axis)?;
            }
            Ok(())
        }
        MapLight::Point {
            translation,
            color,
            candela,
            range,
        } => {
            ensure!(
                matches!(punctual.kind(), gltf::khr_lights_punctual::Kind::Point),
                "point light node is not a point light"
            );
            ensure!(
                punctual.color() == *color,
                "point light color {:?} differs from the document {:?}",
                punctual.color(),
                color
            );
            ensure!(
                punctual.intensity() == *candela,
                "point light intensity {} differs from the document {candela}",
                punctual.intensity()
            );
            ensure!(
                punctual.range() == Some(*range),
                "point light range {:?} differs from the document {range}",
                punctual.range()
            );

            let (actual, _, _) = node.transform().decomposed();
            ensure_close(
                "point light position",
                ROOT_FIX * Vec3::from(actual),
                *translation,
            )
        }
    }
}

fn validate_prop(node: &gltf::Node, expected: &MapNode<lif::LifProp>) -> anyhow::Result<()> {
    validate_extras(node, lif::EXTRAS_PROP, &expected.payload)?;
    ensure_position(node, expected.translation, &expected.name)?;

    let (_, actual, actual_scale) = node.transform().decomposed();
    ensure_close(
        &format!("node '{}' scale", expected.name),
        Vec3::from(actual_scale),
        expected.scale,
    )?;

    // Undo the fix on both sides to compare the native placement.
    let world = ROOT_FIX * Quat::from_array(actual) * ROOT_FIX;
    let wanted = expected.rotation;
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        ensure_close(
            &format!("node '{}' rotation", expected.name),
            world * axis,
            wanted * axis,
        )?;
    }
    Ok(())
}

fn validate_emitter<T>(node: &gltf::Node, key: &str, expected: &MapNode<T>) -> anyhow::Result<()>
where
    T: serde::de::DeserializeOwned + PartialEq + std::fmt::Debug + serde::Serialize,
{
    validate_extras(node, key, &expected.payload)?;
    ensure_position(node, expected.translation, &expected.name)
}

fn validate_extras<T>(node: &gltf::Node, key: &str, expected: &T) -> anyhow::Result<()>
where
    T: serde::de::DeserializeOwned + PartialEq + std::fmt::Debug + serde::Serialize,
{
    let raw = node
        .extras()
        .as_ref()
        .with_context(|| format!("node '{}' carries no extras", node_name(node)))?;
    let value: serde_json::Value = serde_json::from_str(raw.get())
        .with_context(|| format!("parsing extras of node '{}'", node_name(node)))?;
    let entry = value
        .get(key)
        .with_context(|| format!("node '{}' has no '{key}' extras", node_name(node)))?;
    let actual: T = serde_json::from_value(entry.clone())
        .with_context(|| format!("decoding '{key}' of node '{}'", node_name(node)))?;

    ensure!(
        &actual == expected,
        "node '{}' {key} params {actual:?} differ from the document {expected:?}",
        node_name(node)
    );
    Ok(())
}

fn ensure_position(node: &gltf::Node, world_position: Vec3, name: &str) -> anyhow::Result<()> {
    let (translation, _, _) = node.transform().decomposed();
    ensure_close(
        &format!("node '{name}' position"),
        ROOT_FIX * Vec3::from(translation),
        world_position,
    )
}

fn node_name(node: &gltf::Node) -> String {
    node.name().unwrap_or("<unnamed>").to_string()
}

fn validate_root_extensions(
    root: &gltf_json::Root,
    blob: &[u8],
    doc: &MapDocument,
) -> anyhow::Result<()> {
    validate_gat_extension(root, blob, doc)?;
    validate_map_extension(root, doc)?;
    validate_water_extension(root, blob, doc)
}

/// The strongest guarantee of the format: the bin chunk holds the document's
/// `.gat` file, byte for byte.
fn validate_gat_extension(
    root: &gltf_json::Root,
    blob: &[u8],
    doc: &MapDocument,
) -> anyhow::Result<()> {
    let gat: lif::LifGat = root_extension(root, lif::EXTENSION_GAT)?;
    ensure!(
        gat.width == doc.gat.width && gat.height == doc.gat.height,
        "{} declares {}x{}, the document GAT is {}x{}",
        lif::EXTENSION_GAT,
        gat.width,
        gat.height,
        doc.gat.width,
        doc.gat.height
    );

    let expected = doc.gat.to_bytes().context("serializing the document GAT")?;
    let embedded = gat_bytes(root, blob, &gat)?;
    ensure!(
        embedded == expected,
        "{} bufferView holds {} bytes that differ from the {} document .gat bytes",
        lif::EXTENSION_GAT,
        embedded.len(),
        expected.len()
    );
    gat.validate(embedded)
        .with_context(|| format!("validating {} dims", lif::EXTENSION_GAT))
}

fn validate_map_extension(root: &gltf_json::Root, doc: &MapDocument) -> anyhow::Result<()> {
    let map: lif::LifMap = root_extension(root, lif::EXTENSION_MAP)?;
    ensure!(
        map == doc.meta,
        "{} {map:?} differs from the document {:?}",
        lif::EXTENSION_MAP,
        doc.meta
    );
    Ok(())
}

/// A glb declares this extension exactly when the document has water.
fn validate_water_extension(
    root: &gltf_json::Root,
    blob: &[u8],
    doc: &MapDocument,
) -> anyhow::Result<()> {
    let declared = root
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.others.get(lif::EXTENSION_WATER))
        .is_some();

    let Some(expected) = &doc.water else {
        ensure!(
            !declared,
            "glb declares {} but the document has no water",
            lif::EXTENSION_WATER
        );
        return Ok(());
    };

    let water: lif::LifWater = root_extension(root, lif::EXTENSION_WATER)?;
    validate_water_mask(root, blob, &water, expected)?;

    let mut expected = expected.zones.clone();
    expected.buffer_view = water.buffer_view;
    ensure!(
        water == expected,
        "{} {water:?} differs from the document water {expected:?}",
        lif::EXTENSION_WATER
    );
    Ok(())
}

fn validate_water_mask(
    root: &gltf_json::Root,
    blob: &[u8],
    water: &lif::LifWater,
    expected: &MapWater,
) -> anyhow::Result<()> {
    let width = expected.zones.width as usize;
    let height = expected.zones.height as usize;
    ensure!(
        water.width as usize == width && water.height as usize == height,
        "{} dimensions {}x{} differ from the document {}x{}",
        lif::EXTENSION_WATER,
        water.width,
        water.height,
        width,
        height
    );

    let embedded = water_mask_bytes(root, blob, water)?;
    ensure!(
        embedded.len() == (width * height).div_ceil(8),
        "{} bufferView holds {} bytes, expected {} for {width}x{height}",
        lif::EXTENSION_WATER,
        embedded.len(),
        (width * height).div_ceil(8)
    );
    ensure!(
        lif::decode_water_mask(embedded, width, height) == expected.tiles,
        "{} mask differs from the document's selected tiles",
        lif::EXTENSION_WATER
    );
    Ok(())
}

fn water_mask_bytes<'a>(
    root: &gltf_json::Root,
    blob: &'a [u8],
    water: &lif::LifWater,
) -> anyhow::Result<&'a [u8]> {
    let view = root.buffer_views.get(water.buffer_view).with_context(|| {
        format!(
            "LIF_water points at missing bufferView {}",
            water.buffer_view
        )
    })?;
    let start = view.byte_offset.map_or(0, |offset| offset.0 as usize);
    let end = start + view.byte_length.0 as usize;
    if end > blob.len() {
        bail!(
            "LIF_water bufferView spans {start}..{end}, past the {} byte BIN chunk",
            blob.len()
        );
    }
    Ok(&blob[start..end])
}

fn gat_bytes<'a>(
    root: &gltf_json::Root,
    blob: &'a [u8],
    gat: &lif::LifGat,
) -> anyhow::Result<&'a [u8]> {
    let view = root
        .buffer_views
        .get(gat.buffer_view as usize)
        .with_context(|| format!("LIF_gat points at missing bufferView {}", gat.buffer_view))?;
    let start = view.byte_offset.map_or(0, |offset| offset.0 as usize);
    let end = start + view.byte_length.0 as usize;
    if end > blob.len() {
        bail!(
            "LIF_gat bufferView spans {start}..{end}, past the {} byte BIN chunk",
            blob.len()
        );
    }
    Ok(&blob[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::map::fixtures::{Fixture, write_fixture};
    use ro_formats::RswObject;

    #[test]
    fn a_freshly_written_glb_validates_against_its_sources() {
        let fixture = write_fixture();

        let counts = validate(&fixture.path, &fixture.document()).expect("validation passes");

        assert_eq!(
            counts,
            Counts {
                meshes: 1,
                lights: 2,
                sounds: 1,
                effects: 1,
                props: 1,
            }
        );
        assert_eq!(
            counts.to_string(),
            "1 meshes, 2 lights, 1 sounds, 1 effects, 1 props"
        );
    }

    fn assert_fails(fixture: &Fixture, expected: &str) {
        let err = validate(&fixture.path, &fixture.document()).expect_err("validation must fail");
        let message = format!("{err:#}");
        assert!(
            message.contains(expected),
            "expected an error mentioning '{expected}', got: {message}"
        );
    }

    #[test]
    fn mutated_gat_bytes_fail_the_byte_for_byte_check() {
        let mut fixture = write_fixture();
        fixture.gat[24] ^= 0xff;

        assert_fails(&fixture, lif::EXTENSION_GAT);
    }

    #[test]
    fn a_source_that_no_longer_hashes_to_the_recorded_provenance_fails() {
        let mut fixture = write_fixture();
        fixture.gnd_bytes = b"other-gnd-bytes".to_vec();

        assert_fails(&fixture, lif::EXTENSION_MAP);
    }

    #[test]
    fn converter_output_without_provenance_is_rejected() {
        let fixture = write_fixture();
        let patched = crate::converters::map::fixtures::patched_glb(
            &fixture.path,
            "\"source\":{\"gat_hash\"",
            "\"Xource\":{\"gat_hash\"",
        );
        std::fs::write(&fixture.path, patched).expect("rewrite glb");

        assert_fails(&fixture, lif::EXTENSION_MAP);
    }

    /// Retail map names carry `@` and `-`; both the document and the written
    /// glb must accept them.
    #[test]
    fn retail_map_names_with_at_signs_and_dashes_still_validate() {
        let fixture = write_fixture();
        let mut document = fixture.document();
        document.name = "pvp_y_1-2".to_string();

        crate::converters::map::writer::write_glb(&fixture.path, &document).expect("rewrite glb");
        let counts =
            super::validate(&fixture.path, &document).expect("a retail map name must validate");

        assert_eq!(counts.lights, 2);
    }

    #[test]
    fn water_params_that_drifted_from_the_rsw_fail() {
        let mut fixture = write_fixture();
        fixture.world.water.wave_height += 1.0;

        assert_fails(&fixture, lif::EXTENSION_WATER);
    }

    #[test]
    fn a_water_mask_that_disagrees_with_the_gnd_selection_fails() {
        let mut fixture = write_fixture();
        fixture.ground.surfaces[0].height = [12.0; 4];

        assert_fails(&fixture, "mask differs");
    }

    #[test]
    fn a_map_that_lost_its_water_since_writing_fails() {
        let mut fixture = write_fixture();
        fixture.world.water.level = 0.0;

        assert_fails(&fixture, lif::EXTENSION_WATER);
    }

    #[test]
    fn drifted_ambient_color_fails() {
        let mut fixture = write_fixture();
        fixture.world.light.ambient[1] += 0.1;

        assert_fails(&fixture, lif::EXTENSION_MAP);
    }

    #[test]
    fn a_sun_pointing_elsewhere_than_the_native_computation_fails() {
        let mut fixture = write_fixture();
        fixture.world.light.latitude += 20;

        assert_fails(&fixture, "sun direction");
    }

    #[test]
    fn a_moved_point_light_fails() {
        let mut fixture = write_fixture();
        let RswObject::Light(light) = &mut fixture.world.objects[0] else {
            panic!("fixture object 0 is the point light");
        };
        light.position[0] += 5.0;

        assert_fails(&fixture, "point light position");
    }

    #[test]
    fn drifted_emitter_params_fail() {
        let mut fixture = write_fixture();
        let RswObject::Sound(sound) = &mut fixture.world.objects[1] else {
            panic!("fixture object 1 is the sound");
        };
        sound.volume += 0.1;

        assert_fails(&fixture, lif::EXTRAS_AUDIO);
    }

    #[test]
    fn a_moved_prop_fails() {
        let mut fixture = write_fixture();
        let RswObject::Model(model) = &mut fixture.world.objects[3] else {
            panic!("fixture object 3 is the prop");
        };
        model.position[2] -= 3.0;

        assert_fails(&fixture, "'tree' position");
    }

    #[test]
    fn an_rsw_that_gained_an_object_since_writing_fails() {
        let mut fixture = write_fixture();
        let extra = fixture.world.objects[2].clone();
        fixture.world.objects.push(extra);

        assert_fails(&fixture, "object nodes");
    }

    #[test]
    fn a_terrain_that_lost_primitives_fails() {
        let mut fixture = write_fixture();
        let extra = fixture.primitives[0].clone();
        fixture.primitives.push(extra);

        assert_fails(&fixture, "primitives");
    }

    #[test]
    fn a_truncated_terrain_primitive_fails() {
        let mut fixture = write_fixture();
        fixture.primitives[0].positions.push([0.0, 0.0, 0.0]);

        assert_fails(&fixture, "vertices");
    }
}
