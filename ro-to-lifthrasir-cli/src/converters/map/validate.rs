//! Re-reads a written `.glb` and proves it still says what the sources said.
//!
//! Nothing here trusts the writer's in-memory state: the file is parsed back
//! from disk and every value is compared against the parsed RSW/GND/GAT (and,
//! for the sun, against the native lighting math). Any mismatch is an error --
//! a truncated or silently wrong map must never reach the pak.

use crate::converters::gltf_out::{ensure_close, root_extension, scene_root};
use crate::converters::map::terrain::TerrainPrimitive;
use crate::converters::map::water;
use crate::converters::map::writer::{self, MapGlbInputs, ROOT_FIX};
use anyhow::{Context, bail, ensure};
use glam::{Quat, Vec3};
use lifthrasir_data::lif;
use ro_formats::{RswLight, RswLightObj, RswObject};
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

/// Re-read `glb_path` and assert it round-trips `inputs`.
pub fn validate(glb_path: &Path, inputs: &MapGlbInputs) -> anyhow::Result<Counts> {
    let gltf::Gltf { document, blob } =
        gltf::Gltf::open(glb_path).with_context(|| format!("re-reading {}", glb_path.display()))?;
    let blob = blob.with_context(|| format!("{} has no BIN chunk", glb_path.display()))?;

    let counts = {
        let root = scene_root(&document)?;
        validate_nodes(&root, &blob, inputs)?
    };
    validate_root_extensions(&document.into_json(), &blob, inputs)?;

    Ok(counts)
}

/// The scene root's children are, in order: the terrain node (when the map has
/// geometry), the sun, then one node per emitted RSW object. Walking them in
/// that order validates placement and ordering at once.
fn validate_nodes(root: &gltf::Node, blob: &[u8], inputs: &MapGlbInputs) -> anyhow::Result<Counts> {
    let children: Vec<gltf::Node> = root.children().collect();
    let mut next = 0;

    let primitives: Vec<&TerrainPrimitive> = inputs
        .primitives
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

    let sun = children.get(next).context("glb has no sun node")?;
    next += 1;
    validate_sun(sun, &inputs.world.light)?;

    let objects: Vec<&RswObject> = writer::emitted_objects(inputs.world).collect();
    let emitted = &children[next..];
    ensure!(
        emitted.len() == objects.len(),
        "glb has {} object nodes, the RSW emits {}",
        emitted.len(),
        objects.len()
    );

    let mut counts = Counts {
        meshes,
        lights: 1,
        ..Default::default()
    };
    let map_width = inputs.ground.width as f32;
    let map_height = inputs.ground.height as f32;

    for (node, object) in emitted.iter().zip(objects) {
        match object {
            RswObject::Light(light) => {
                validate_point_light(node, light, map_width, map_height)?;
                counts.lights += 1;
            }
            RswObject::Sound(sound) => {
                validate_extras(node, lif::EXTRAS_AUDIO, &writer::lif_audio(sound))?;
                ensure_position(node, sound.position, map_width, map_height, &sound.name)?;
                counts.sounds += 1;
            }
            RswObject::Effect(effect) => {
                validate_extras(node, lif::EXTRAS_EFFECT, &writer::lif_effect(effect))?;
                ensure_position(node, effect.position, map_width, map_height, &effect.name)?;
                counts.effects += 1;
            }
            RswObject::Model(model) => {
                validate_extras(
                    node,
                    lif::EXTRAS_PROP,
                    &writer::lif_prop(model, inputs.converted_models)?,
                )?;
                ensure_position(node, model.position, map_width, map_height, &model.name)?;
                counts.props += 1;
            }
        }
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
        "glb terrain has {} primitives, the GND builds {}",
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
            "primitive '{}' has {positions} vertices, the GND builds {}",
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
            "primitive '{}' has {indices} indices, the GND builds {}",
            expected.texture,
            expected.indices.len()
        );
    }

    Ok(primitives.len())
}

fn validate_sun(node: &gltf::Node, light: &RswLight) -> anyhow::Result<()> {
    let punctual = node.light().context("sun node carries no KHR light")?;
    ensure!(
        matches!(
            punctual.kind(),
            gltf::khr_lights_punctual::Kind::Directional
        ),
        "sun node is not a directional light"
    );
    ensure!(
        punctual.color() == light.diffuse,
        "sun color {:?} differs from the RSW diffuse {:?}",
        punctual.color(),
        light.diffuse
    );

    let (_, rotation, _) = node.transform().decomposed();
    let world_direction = (ROOT_FIX * Quat::from_array(rotation)) * Vec3::NEG_Z;
    ensure_close(
        "sun direction",
        world_direction,
        writer::sun_direction(light),
    )
}

fn validate_point_light(
    node: &gltf::Node,
    light: &RswLightObj,
    map_width: f32,
    map_height: f32,
) -> anyhow::Result<()> {
    let punctual = node
        .light()
        .with_context(|| format!("light '{}' carries no KHR light", light.name))?;
    ensure!(
        matches!(punctual.kind(), gltf::khr_lights_punctual::Kind::Point),
        "light '{}' is not a point light",
        light.name
    );
    ensure!(
        punctual.color() == light.color,
        "light '{}' color {:?} differs from the RSW {:?}",
        light.name,
        punctual.color(),
        light.color
    );
    ensure!(
        punctual.range() == Some(light.range),
        "light '{}' range {:?} differs from the RSW {}",
        light.name,
        punctual.range(),
        light.range
    );

    ensure_position(node, light.position, map_width, map_height, &light.name)
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
        "node '{}' {key} params {actual:?} differ from the RSW {expected:?}",
        node_name(node)
    );
    Ok(())
}

fn ensure_position(
    node: &gltf::Node,
    rsw_position: [f32; 3],
    map_width: f32,
    map_height: f32,
    name: &str,
) -> anyhow::Result<()> {
    let (translation, _, _) = node.transform().decomposed();
    ensure_close(
        &format!("node '{name}' position"),
        ROOT_FIX * Vec3::from(translation),
        writer::rsw_position_to_world(rsw_position, map_width, map_height),
    )
}

fn node_name(node: &gltf::Node) -> String {
    node.name().unwrap_or("<unnamed>").to_string()
}

fn validate_root_extensions(
    root: &gltf_json::Root,
    blob: &[u8],
    inputs: &MapGlbInputs,
) -> anyhow::Result<()> {
    validate_gat_extension(root, blob, inputs.gat_bytes)?;
    validate_map_extension(root, inputs)?;
    validate_water_extension(root, blob, inputs)
}

/// The strongest guarantee of the format: the bin chunk still holds the
/// server's `.gat` file, byte for byte.
fn validate_gat_extension(
    root: &gltf_json::Root,
    blob: &[u8],
    source: &[u8],
) -> anyhow::Result<()> {
    let gat: lif::LifGat = root_extension(root, lif::EXTENSION_GAT)?;
    let embedded = gat_bytes(root, blob, &gat)?;
    ensure!(
        embedded == source,
        "{} bufferView holds {} bytes that differ from the {} source .gat bytes",
        lif::EXTENSION_GAT,
        embedded.len(),
        source.len()
    );
    gat.validate(embedded)
        .with_context(|| format!("validating {} dims", lif::EXTENSION_GAT))
}

fn validate_map_extension(root: &gltf_json::Root, inputs: &MapGlbInputs) -> anyhow::Result<()> {
    let map: lif::LifMap = root_extension(root, lif::EXTENSION_MAP)?;
    ensure!(
        map.format_version == lif::FORMAT_VERSION,
        "{} format_version is {}, this converter writes {}",
        lif::EXTENSION_MAP,
        map.format_version,
        lif::FORMAT_VERSION
    );

    for (label, actual, source) in [
        ("rsw", &map.rsw_hash, inputs.rsw_bytes),
        ("gnd", &map.gnd_hash, inputs.gnd_bytes),
        ("gat", &map.gat_hash, inputs.gat_bytes),
    ] {
        let expected = writer::hash_hex(source);
        ensure!(
            *actual == expected,
            "{} {label}_hash {actual} differs from the source hash {expected}",
            lif::EXTENSION_MAP
        );
    }

    ensure!(
        map.ambient_color == inputs.world.light.ambient,
        "{} ambient {:?} differs from the RSW {:?}",
        lif::EXTENSION_MAP,
        map.ambient_color,
        inputs.world.light.ambient
    );
    let expected_tint = lif::no_shade_tint(inputs.world.light.ambient, inputs.world.light.diffuse);
    ensure!(
        map.no_shade_tint == expected_tint,
        "{} no-shade tint {:?} differs from the RSW-derived {:?}",
        lif::EXTENSION_MAP,
        map.no_shade_tint,
        expected_tint
    );
    Ok(())
}

/// The writer omits this extension when every resolved zone has level zero;
/// a glb that carries one anyway is wrong.
fn validate_water_extension(
    root: &gltf_json::Root,
    blob: &[u8],
    inputs: &MapGlbInputs,
) -> anyhow::Result<()> {
    let declared = root
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.others.get(lif::EXTENSION_WATER))
        .is_some();

    let mut expected = water::resolve_water(inputs.ground, &inputs.world.water)?;
    if expected.zones.iter().all(|zone| zone.level == 0.0) {
        ensure!(
            !declared,
            "glb declares {} but the resolved water zones have no water level",
            lif::EXTENSION_WATER
        );
        return Ok(());
    }

    let water: lif::LifWater = root_extension(root, lif::EXTENSION_WATER)?;
    expected.buffer_view = water.buffer_view;
    ensure!(
        water == expected,
        "{} {water:?} differs from the resolved source water {expected:?}",
        lif::EXTENSION_WATER
    );

    let width = inputs.ground.width as usize;
    let height = inputs.ground.height as usize;
    ensure!(
        water.width == inputs.ground.width && water.height == inputs.ground.height,
        "{} dimensions {}x{} differ from the GND {}x{}",
        lif::EXTENSION_WATER,
        water.width,
        water.height,
        inputs.ground.width,
        inputs.ground.height
    );
    let embedded = water_mask_bytes(root, blob, &water)?;
    let expected_tiles = water::select_water_tiles(inputs.ground, &expected);
    ensure!(
        embedded.len() == (width * height).div_ceil(8),
        "{} bufferView holds {} bytes, expected {} for {width}x{height}",
        lif::EXTENSION_WATER,
        embedded.len(),
        (width * height).div_ceil(8)
    );
    ensure!(
        lif::decode_water_mask(embedded, width, height) == expected_tiles,
        "{} mask differs from the GND-corner selection",
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

    #[test]
    fn a_freshly_written_glb_validates_against_its_sources() {
        let fixture = write_fixture();

        let counts = validate(&fixture.path, &fixture.inputs()).expect("validation passes");

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
        let err = validate(&fixture.path, &fixture.inputs()).expect_err("validation must fail");
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

        assert_fails(&fixture, "gnd_hash");
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

        assert_fails(&fixture, "ambient");
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

        assert_fails(&fixture, "'torch' position");
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
