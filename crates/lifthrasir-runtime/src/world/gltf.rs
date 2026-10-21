//! Decode LIF root extensions and node metadata without client map-selection policy.
use super::components::*;
use bevy::{
    asset::LoadContext,
    gltf::{
        GltfLoaderSettings,
        extensions::{ErasedGltfExtensionHandler, GltfExtensionHandler, GltfExtensionHandlers},
    },
    light::CascadeShadowConfigBuilder,
    prelude::*,
};
use lifthrasir_data::lif::{self, LifEditor, LifMap, LifWater};

/// Register LIF metadata decoding after Bevy's glTF plugin; installs no map lifecycle.
pub struct MapGltfPlugin;
impl Plugin for MapGltfPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut()
            .get_resource_mut::<GltfExtensionHandlers>()
            .expect("MapGltfPlugin requires Bevy glTF infrastructure before registration")
            .0
            .write_blocking()
            .push(Box::new(LifExtensionHandler::default()));
        app.register_type::<LifMapData>()
            .register_type::<LifMapLoadError>()
            .register_type::<LifAudioEmitter>()
            .register_type::<LifEffectEmitter>()
            .register_type::<LifPropRef>()
            .register_type::<LifMapNodeIndex>();
    }
}

/// Map directional-light tuning. Colour and illuminance already arrive through
/// `KHR_lights_punctual`.
const SUN_SHADOW_DEPTH_BIAS: f32 = 0.02;
const SUN_SHADOW_NORMAL_BIAS: f32 = 1.8;
const SUN_CASCADES: usize = 3;
const SUN_FIRST_CASCADE_FAR_BOUND: f32 = 200.0;
const SUN_MAXIMUM_DISTANCE: f32 = 1500.0;
const SUN_OVERLAP_PROPORTION: f32 = 0.2;

/// Map point-light tuning: a wide emitter surface that softens the
/// inverse-square hot spot near the source.
const POINT_LIGHT_RADIUS: f32 = 1.5;

/// Per-load state: the root extensions decoded once in [`on_root`], handed to
/// the scene root in [`on_scene_completed`].
///
/// `None` means the document carries no `LIF_map` and is therefore not a
/// Lifthrasir map -- every other hook then leaves it alone.
///
/// [`on_root`]: GltfExtensionHandler::on_root
/// [`on_scene_completed`]: GltfExtensionHandler::on_scene_completed
#[derive(Clone, Default)]
struct LifExtensionHandler {
    state: Option<Result<LifMapData, String>>,
}

impl LifExtensionHandler {
    /// Whether this load is a Lifthrasir map at all, broken or not.
    fn is_map(&self) -> bool {
        self.state.is_some()
    }

    /// Whether this load is a Lifthrasir map that decoded cleanly.
    fn is_usable_map(&self) -> bool {
        matches!(self.state, Some(Ok(_)))
    }

    /// Records the first failure of this load and logs it. Later failures are
    /// logged too but do not replace the root cause.
    fn fail(&mut self, load_context: &LoadContext<'_>, message: String) {
        error!("{}: {message}", load_context.path());
        if !matches!(self.state, Some(Err(_))) {
            self.state = Some(Err(message));
        }
    }
}

impl GltfExtensionHandler for LifExtensionHandler {
    fn dyn_clone(&self) -> Box<dyn ErasedGltfExtensionHandler> {
        Box::new(self.clone())
    }

    fn on_root(
        &mut self,
        load_context: &mut LoadContext<'_>,
        gltf: &gltf::Gltf,
        _settings: &GltfLoaderSettings,
    ) {
        if gltf.extension_value(lif::EXTENSION_MAP).is_none() {
            return;
        }

        match decode_root(gltf) {
            Ok(data) => self.state = Some(Ok(data)),
            Err(message) => self.fail(load_context, message),
        }
    }

    fn on_gltf_node(
        &mut self,
        load_context: &mut LoadContext<'_>,
        gltf_node: &gltf::Node,
        entity: &mut EntityWorldMut,
    ) {
        if !self.is_map() {
            return;
        }
        entity.insert(LifMapNodeIndex(gltf_node.index()));

        let Some(extras) = gltf_node.extras().as_ref() else {
            return;
        };

        if let Err(message) = insert_node_markers(extras.get(), entity) {
            let name = gltf_node.name().unwrap_or("<unnamed>");
            self.fail(
                load_context,
                format!("node {} '{name}': {message}", gltf_node.index()),
            );
        }
    }

    fn on_scene_completed(
        &mut self,
        _load_context: &mut LoadContext<'_>,
        _scene: &gltf::Scene,
        world_root_id: Entity,
        scene_world: &mut World,
    ) {
        let mut root = scene_world.entity_mut(world_root_id);
        match &self.state {
            Some(Ok(data)) => {
                root.insert(data.clone());
            }
            Some(Err(message)) => {
                root.insert(LifMapLoadError(message.clone()));
            }
            None => {}
        }
    }

    fn on_spawn_light_directional(
        &mut self,
        _load_context: &mut LoadContext<'_>,
        _gltf_node: &gltf::Node,
        entity: &mut EntityWorldMut,
    ) {
        if !self.is_usable_map() {
            return;
        }

        if let Some(mut light) = entity.get_mut::<DirectionalLight>() {
            light.shadow_maps_enabled = true;
            light.shadow_depth_bias = SUN_SHADOW_DEPTH_BIAS;
            light.shadow_normal_bias = SUN_SHADOW_NORMAL_BIAS;
        }

        entity.insert(
            CascadeShadowConfigBuilder {
                num_cascades: SUN_CASCADES,
                first_cascade_far_bound: SUN_FIRST_CASCADE_FAR_BOUND,
                maximum_distance: SUN_MAXIMUM_DISTANCE,
                overlap_proportion: SUN_OVERLAP_PROPORTION,
                ..default()
            }
            .build(),
        );
    }

    fn on_spawn_light_point(
        &mut self,
        _load_context: &mut LoadContext<'_>,
        _gltf_node: &gltf::Node,
        entity: &mut EntityWorldMut,
    ) {
        if !self.is_usable_map() {
            return;
        }

        if let Some(mut light) = entity.get_mut::<PointLight>() {
            light.radius = POINT_LIGHT_RADIUS;
            light.shadow_maps_enabled = false;
        }
    }
}

fn decode_root(gltf: &gltf::Gltf) -> Result<LifMapData, String> {
    let meta: LifMap = root_extension(gltf, lif::EXTENSION_MAP)?
        .ok_or_else(|| format!("missing root extension {}", lif::EXTENSION_MAP))?;
    if meta.format_version != lif::FORMAT_VERSION {
        return Err(format!(
            "{} format_version {} is not supported (runtime expects {})",
            lif::EXTENSION_MAP,
            meta.format_version,
            lif::FORMAT_VERSION
        ));
    }

    let editor: LifEditor = root_extension(gltf, lif::EXTENSION_EDITOR)?
        .ok_or_else(|| format!("missing root extension {}", lif::EXTENSION_EDITOR))?;
    let collision = editor.collision;
    let collision_bytes = buffer_view_bytes(gltf, collision.buffer_view, lif::EXTENSION_EDITOR)?;
    let altitude = collision
        .to_altitude(collision_bytes)
        .map_err(|error| format!("decoding {} collision: {error}", lif::EXTENSION_EDITOR))?;

    let water = root_extension::<LifWater>(gltf, lif::EXTENSION_WATER)?;
    let water_tiles = match water.as_ref() {
        Some(water) => {
            let zone_count = water
                .split_width
                .checked_mul(water.split_height)
                .ok_or_else(|| format!("{} zone dimensions overflow", lif::EXTENSION_WATER))?
                as usize;
            if water.split_width == 0 || water.split_height == 0 || water.zones.len() != zone_count
            {
                return Err(format!(
                    "{} declares a {}x{} zone grid but carries {} zones",
                    lif::EXTENSION_WATER,
                    water.split_width,
                    water.split_height,
                    water.zones.len()
                ));
            }

            let mask = buffer_view_bytes(gltf, water.buffer_view, lif::EXTENSION_WATER)?;
            let cell_count = (water.width as usize)
                .checked_mul(water.height as usize)
                .ok_or_else(|| format!("{} mask dimensions overflow", lif::EXTENSION_WATER))?;
            let expected_mask_len = cell_count.div_ceil(8);
            if mask.len() != expected_mask_len {
                return Err(format!(
                    "{} mask has {} bytes but {}x{} dimensions require {expected_mask_len}",
                    lif::EXTENSION_WATER,
                    mask.len(),
                    water.width,
                    water.height,
                ));
            }
            lif::decode_water_mask(mask, water.width as usize, water.height as usize)
        }
        None => Vec::new(),
    };

    Ok(LifMapData {
        altitude,
        water,
        water_tiles,
        meta,
    })
}

fn root_extension<T: serde::de::DeserializeOwned>(
    gltf: &gltf::Gltf,
    key: &str,
) -> Result<Option<T>, String> {
    let Some(value) = gltf.extension_value(key) else {
        return Ok(None);
    };

    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|error| format!("root extension {key} is malformed: {error}"))
}

/// Resolves a `LIF_*` bufferView against the GLB binary chunk.
fn buffer_view_bytes<'a>(
    gltf: &'a gltf::Gltf,
    buffer_view: usize,
    extension: &str,
) -> Result<&'a [u8], String> {
    let view = gltf
        .views()
        .nth(buffer_view)
        .ok_or_else(|| format!("{extension} points at missing bufferView {buffer_view}"))?;

    if !matches!(view.buffer().source(), gltf::buffer::Source::Bin) {
        return Err(format!(
            "{extension} bufferView {buffer_view} is not backed by the GLB binary chunk"
        ));
    }

    let blob = gltf
        .blob
        .as_deref()
        .ok_or_else(|| format!("glb has no binary chunk to read {extension} from"))?;
    let start = view.offset();
    let end = start + view.length();

    blob.get(start..end).ok_or_else(|| {
        format!(
            "{extension} bufferView {start}..{end} is outside the {}-byte binary chunk",
            blob.len()
        )
    })
}

fn insert_node_markers(raw_extras: &str, entity: &mut EntityWorldMut) -> Result<(), String> {
    let extras: serde_json::Value = serde_json::from_str(raw_extras)
        .map_err(|error| format!("extras are not JSON: {error}"))?;

    if let Some(value) = extras.get(lif::EXTRAS_AUDIO) {
        entity.insert(LifAudioEmitter(parse_extras(lif::EXTRAS_AUDIO, value)?));
    }
    if let Some(value) = extras.get(lif::EXTRAS_EFFECT) {
        entity.insert(LifEffectEmitter(parse_extras(lif::EXTRAS_EFFECT, value)?));
    }
    if let Some(value) = extras.get(lif::EXTRAS_PROP) {
        entity.insert(LifPropRef(parse_extras(lif::EXTRAS_PROP, value)?));
    }

    Ok(())
}

fn parse_extras<T: serde::de::DeserializeOwned>(
    key: &str,
    value: &serde_json::Value,
) -> Result<T, String> {
    serde_json::from_value(value.clone())
        .map_err(|error| format!("extras key '{key}' is malformed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::{AssetApp, AssetPlugin},
        gltf::{GltfAssetLabel, GltfPlugin},
        image::{CompressedImageFormats, ImageLoader, ImagePlugin},
        light::CascadeShadowConfig,
        mesh::MeshPlugin,
        world_serialization::{WorldAsset, WorldSerializationPlugin},
    };
    use lifthrasir_data::lif::{LifAudio, LifEffect, LifProp};
    use std::time::{Duration, Instant};
    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: FIXTURES.to_string(),
                ..default()
            },
            ImagePlugin::default(),
            MeshPlugin,
            WorldSerializationPlugin,
            GltfPlugin::default(),
            MapGltfPlugin,
        ));
        app.register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE));
        app.init_asset::<StandardMaterial>();
        // `GltfPlugin` only registers the real loader in `finish`, which
        // `App::update` never calls on its own.
        app.finish();
        app.cleanup();
        app
    }

    /// Loads scene 0 of `file` and blocks until the asset server settles.
    fn load_scene(app: &mut App, file: &str) -> Handle<WorldAsset> {
        let handle = app
            .world()
            .resource::<AssetServer>()
            .load(GltfAssetLabel::Scene(0).from_asset(file.to_string()));

        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            app.update();
            let state = app.world().resource::<AssetServer>().load_state(&handle);
            assert!(!state.is_failed(), "{file} failed to load: {state:?}");
            if state.is_loaded() {
                return handle;
            }
            assert!(Instant::now() < deadline, "{file} never finished loading");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Runs `assertions` against the loaded scene's world. The world lives
    /// inside the asset, so it is reached through `Assets<WorldAsset>`.
    fn with_scene_world(app: &mut App, file: &str, assertions: impl FnOnce(&mut World)) {
        let handle = load_scene(app, file);
        let mut assets = app.world_mut().resource_mut::<Assets<WorldAsset>>();
        let mut asset = assets.get_mut(&handle).expect("scene world asset");
        assertions(&mut asset.world);
    }

    fn single<C: Component + Clone>(world: &mut World) -> C {
        let mut query = world.query::<&C>();
        let mut found = query.iter(world);
        let value = found
            .next()
            .unwrap_or_else(|| panic!("no entity has {}", core::any::type_name::<C>()))
            .clone();
        assert!(
            found.next().is_none(),
            "more than one entity has {}",
            core::any::type_name::<C>()
        );
        value
    }

    #[test]
    fn decodes_the_gat_from_the_glb_binary_chunk() {
        with_scene_world(&mut test_app(), "mini_map.glb", |world| {
            let data = single::<LifMapData>(world);

            assert_eq!(data.meta.format_version, lif::FORMAT_VERSION);
            assert_eq!(data.meta.ambient_color, [0.25, 0.3, 0.35]);

            // The fixture GAT is 4x4 with cell `i` at heights `i..i+3` and
            // raw cell type 1 (non-walkable).
            assert_eq!((data.altitude.width, data.altitude.height), (4, 4));
            assert_eq!(data.altitude.cells.len(), 16);
            assert_eq!(data.altitude.cells[0].height, [0.0, 1.0, 2.0, 3.0]);
            assert_eq!(data.altitude.cells[15].height, [15.0, 16.0, 17.0, 18.0]);
            assert!(!data.altitude.cells[7].cell_type.is_walkable());
        });
    }

    #[test]
    fn carries_the_water_extension_onto_the_scene_root() {
        with_scene_world(&mut test_app(), "mini_map.glb", |world| {
            let water = single::<LifMapData>(world).water.expect("LIF_water");

            assert_eq!((water.split_width, water.split_height), (1, 1));
            let zone = water.zone_at(0, 0);
            assert_eq!(zone.level, 12.5);
            assert_eq!(zone.water_type, 3);
            assert_eq!(zone.wave_height, 0.6);
            assert_eq!(zone.wave_speed, 1.25);
            assert_eq!(zone.wave_pitch, 45.0);
            assert_eq!(zone.anim_speed, 4);
            assert_eq!((water.width, water.height), (2, 2));
        });
    }

    #[test]
    fn a_water_mask_with_incompatible_dimensions_fails_root_decode() {
        let bytes = std::fs::read(format!("{FIXTURES}/bad_water_mask.glb")).unwrap();
        let gltf = gltf::Gltf::from_slice(&bytes).unwrap();

        assert!(
            decode_root(&gltf)
                .unwrap_err()
                .contains("LIF_water mask has 1 bytes but 9x2 dimensions require 3")
        );
    }

    #[test]
    fn turns_node_extras_into_emitter_and_prop_markers() {
        with_scene_world(&mut test_app(), "mini_map.glb", |world| {
            assert_eq!(
                single::<LifAudioEmitter>(world).0,
                LifAudio {
                    name: "water".to_string(),
                    file: "ro://data/wav/effect/water.wav".to_string(),
                    volume: 0.75,
                    range: 30.0,
                    cycle: 4.5,
                }
            );
            assert_eq!(
                single::<LifEffectEmitter>(world).0,
                LifEffect {
                    name: "steam".to_string(),
                    effect_type: 12,
                    emit_speed: 0.25,
                    params: [1.0, 2.0, 3.0, 4.0],
                }
            );
            assert_eq!(
                single::<LifPropRef>(world).0,
                LifProp {
                    model: "ro://models/prontera/tree01.glb".to_string(),
                    anim_type: 1,
                    anim_speed: 2.0,
                }
            );
        });
    }

    #[test]
    fn map_lights_get_shadow_tuning() {
        with_scene_world(&mut test_app(), "mini_map.glb", |world| {
            let sun = single::<DirectionalLight>(world);
            assert!(sun.shadow_maps_enabled);
            assert_eq!(sun.shadow_depth_bias, SUN_SHADOW_DEPTH_BIAS);
            assert_eq!(sun.shadow_normal_bias, SUN_SHADOW_NORMAL_BIAS);
            assert_eq!(sun.illuminance, 4500.0);

            let cascades = single::<CascadeShadowConfig>(world);
            assert_eq!(cascades.bounds.len(), SUN_CASCADES);
            assert_eq!(cascades.overlap_proportion, SUN_OVERLAP_PROPORTION);
            // The builder derives the bounds geometrically, so they land a
            // rounding step off the values fed in.
            assert!((cascades.bounds[0] - SUN_FIRST_CASCADE_FAR_BOUND).abs() < 0.01);
            assert!((cascades.bounds[2] - SUN_MAXIMUM_DISTANCE).abs() < 0.01);

            let point = single::<PointLight>(world);
            assert_eq!(point.radius, POINT_LIGHT_RADIUS);
            assert!(!point.shadow_maps_enabled);
            assert_eq!(point.range, 40.0);
        });
    }

    #[test]
    fn an_unsupported_format_version_marks_the_root_as_failed() {
        with_scene_world(&mut test_app(), "bad_version.glb", |world| {
            let mut query = world.query::<&LifMapData>();
            assert!(
                query.iter(world).next().is_none(),
                "an unsupported LIF_map must not produce map data"
            );

            let error = single::<LifMapLoadError>(world).0;
            assert!(
                error.contains("format_version") && error.contains(lif::EXTENSION_MAP),
                "unhelpful error: {error}"
            );
        });
    }

    /// The handler is global, so ordinary glTF assets travel through it too.
    /// They must come out exactly as Bevy's loader built them.
    #[test]
    fn a_glb_without_lif_extensions_is_left_untouched() {
        with_scene_world(&mut test_app(), "plain.glb", |world| {
            let mut map_data = world.query::<&LifMapData>();
            assert!(map_data.iter(world).next().is_none());
            let mut load_error = world.query::<&LifMapLoadError>();
            assert!(
                load_error.iter(world).next().is_none(),
                "a plain glTF is not a broken map"
            );
            let mut props = world.query::<&LifPropRef>();
            assert!(props.iter(world).next().is_none());

            let sun = single::<DirectionalLight>(world);
            assert!(
                !sun.shadow_maps_enabled,
                "shadow tuning leaked onto a non-map glb"
            );
            assert_eq!(
                single::<CascadeShadowConfig>(world).bounds,
                CascadeShadowConfig::default().bounds
            );
            assert_eq!(single::<PointLight>(world).radius, 0.0);
        });
    }

    #[test]
    fn malformed_node_extras_fail_the_whole_map() {
        with_scene_world(&mut test_app(), "bad_extras.glb", |world| {
            let mut query = world.query::<&LifMapData>();
            assert!(
                query.iter(world).next().is_none(),
                "a malformed lif_audio must not leave usable map data behind"
            );

            let error = single::<LifMapLoadError>(world).0;
            assert!(
                error.contains(lif::EXTRAS_AUDIO),
                "unhelpful error: {error}"
            );
        });
    }
}
