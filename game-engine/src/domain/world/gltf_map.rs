//! Runtime side of the `map-gltf` pipeline: the `LIF_*` glTF extension
//! handler that turns a converted per-map glb back into engine data.
//!
//! The handler is registered globally, so it sees every glb the app loads. A
//! document without a `LIF_map` root extension is not a Lifthrasir map and is
//! left completely untouched -- no components, no light retuning, no logs.
//!
//! Every `GltfExtensionHandler` hook returns `()`, so an extension cannot make
//! Bevy's glTF loader return an error. A malformed or version-incompatible
//! `LIF_*` payload is therefore reported two ways: an `error!` naming the asset
//! path, and a [`LifMapLoadError`] on the scene root *instead of*
//! [`LifMapData`]. Consumers must treat a root without `LifMapData` as a failed
//! map load; there is no partial-map fallback.

use crate::domain::system_sets::WorldLoadingSystems;
use crate::domain::world::components::MapLoader;
use crate::domain::world::map_loader::MapRequestLoader;
use crate::domain::world::map_scoped::MapScoped;
use crate::domain::world::systems::extract_map_from_unified_assets;
use crate::infrastructure::assets::SharedCompositeAssetSource;
use crate::infrastructure::assets::sources::AssetSource;
use bevy::asset::LoadContext;
use bevy::gltf::GltfAssetLabel;
use bevy::gltf::GltfLoaderSettings;
use bevy::gltf::extensions::{
    ErasedGltfExtensionHandler, GltfExtensionHandler, GltfExtensionHandlers,
};
use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};
use lifthrasir_data::lif::{self, LifAudio, LifEffect, LifGat, LifMap, LifProp, LifWater};
use ro_formats::{RoAltitude, RswWater};

/// Native directional-light tuning, mirrored from
/// `presentation/rendering/lighting.rs::setup_directional_light`. Colour and
/// illuminance already arrive through `KHR_lights_punctual`.
const SUN_SHADOW_DEPTH_BIAS: f32 = 0.02;
const SUN_SHADOW_NORMAL_BIAS: f32 = 1.8;
const SUN_CASCADES: usize = 3;
const SUN_FIRST_CASCADE_FAR_BOUND: f32 = 200.0;
const SUN_MAXIMUM_DISTANCE: f32 = 1500.0;
const SUN_OVERLAP_PROPORTION: f32 = 0.2;

/// Native point-light tuning, mirrored from
/// `presentation/rendering/lighting.rs::spawn_point_light`: a wide emitter
/// surface that softens the inverse-square hot spot near the source.
const POINT_LIGHT_RADIUS: f32 = 1.5;

/// The one transform the runtime applies to a map glb: glTF is Y-up, the
/// Lifthrasir world is -Y-up. Everything else (RSW placement) is baked by the
/// converter, which pre-applied this rotation's inverse.
///
/// It must stay bit-identical to `writer::ROOT_FIX` in `ro-to-lifthrasir-cli`:
/// `Quat::from_rotation_x(PI)` is the same rotation on paper but leaves a ~9e-7
/// residue that the converter's validation pass does not expect.
pub const ROOT_FIX: Quat = Quat::from_xyzw(1.0, 0.0, 0.0, 0.0);

/// Map-wide data decoded from the glb's root extensions, on the scene root.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(opaque)]
#[reflect(Component, Debug, Clone)]
pub struct LifMapData {
    pub altitude: RoAltitude,
    pub water: Option<RswWater>,
    pub meta: LifMap,
}

/// On the scene root when the `LIF_*` payload could not be read. Mutually
/// exclusive with [`LifMapData`].
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(opaque)]
#[reflect(Component, Debug, Clone)]
pub struct LifMapLoadError(pub String);

/// A node carrying `lif_audio` extras; its transform is the emitter position.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(opaque)]
#[reflect(Component, Debug, Clone)]
pub struct LifAudioEmitter(pub LifAudio);

/// A node carrying `lif_effect` extras; its transform is the emitter position.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(opaque)]
#[reflect(Component, Debug, Clone)]
pub struct LifEffectEmitter(pub LifEffect);

/// A node carrying `lif_prop` extras; its transform is the baked RSW placement
/// of the referenced native RSM model.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(opaque)]
#[reflect(Component, Debug, Clone)]
pub struct LifPropRef(pub LifProp);

/// On the map-loader entity whose map is served by a converted glb instead of
/// the native `.gnd`/`.gat`/`.rsw` triple. The entity is the scene root, so the
/// whole imported map hangs off it.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component, Debug, Clone)]
pub struct GltfMapLoader {
    /// Lowercased, extension-free map name -- what the glb path was built from.
    pub map_name: String,
}

/// Runtime plugin for the `map-gltf` pipeline (unified per-map glb loading).
pub struct GltfMapPlugin;

impl Plugin for GltfMapPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<LifMapData>()
            .register_type::<LifMapLoadError>()
            .register_type::<LifAudioEmitter>()
            .register_type::<LifEffectEmitter>()
            .register_type::<LifPropRef>()
            .register_type::<GltfMapLoader>();

        app.world_mut()
            .resource_mut::<GltfExtensionHandlers>()
            .0
            .write_blocking()
            .push(Box::new(LifExtensionHandler::default()));

        app.add_systems(
            Update,
            spawn_gltf_map
                .in_set(WorldLoadingSystems::AssetExtraction)
                .before(extract_map_from_unified_assets),
        );
        app.add_observer(enforce_gltf_map_loaded);
    }
}

/// Claims a map load whose glb exists, ahead of the native
/// [`extract_map_from_unified_assets`]: the scene replaces the `.gnd`/`.gat`/
/// `.rsw` triple, and marking the request loaded is what keeps the native
/// system from also running for this map.
///
/// The scene is rooted on the map-loader entity itself, so it inherits the
/// entity's `MapScoped` teardown and is where the native path puts `MapData`.
pub fn spawn_gltf_map(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    source: Res<SharedCompositeAssetSource>,
    mut requests: Query<(Entity, &mut MapRequestLoader), Without<MapLoader>>,
) {
    for (entity, mut request) in requests.iter_mut() {
        if request.loaded {
            continue;
        }

        let map_name = request.map_name.trim_end_matches(".gat").to_lowercase();
        let path = format!("data/maps/{map_name}/{map_name}.glb");

        if !source
            .0
            .read()
            .expect("composite asset source lock is poisoned")
            .exists(&path)
        {
            continue;
        }

        info!("Loading map '{map_name}' from {path}");

        commands.entity(entity).insert((
            WorldAssetRoot(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset(format!("ro://{path}"))),
            ),
            Transform::from_rotation(ROOT_FIX),
            MapScoped,
            GltfMapLoader { map_name },
        ));

        request.loaded = true;
    }
}

/// A glb map that imported badly is a hard failure: bevy's extension hooks
/// cannot fail a load, so a broken `LIF_*` payload still reports `Loaded` and
/// would otherwise spawn a map missing its altitude, water and metadata.
fn enforce_gltf_map_loaded(
    ready: On<WorldInstanceReady>,
    roots: Query<&GltfMapLoader>,
    children: Query<&Children>,
    errors: Query<&LifMapLoadError>,
    map_data: Query<&LifMapData>,
) {
    let Ok(root) = roots.get(ready.entity) else {
        return;
    };

    let map_name = &root.map_name;

    if let Some(error) = children
        .iter_descendants(ready.entity)
        .find_map(|entity| errors.get(entity).ok())
    {
        panic!("map glb for '{map_name}' is unusable: {}", error.0);
    }

    if children
        .iter_descendants(ready.entity)
        .all(|entity| map_data.get(entity).is_err())
    {
        panic!("map glb for '{map_name}' carries no LIF_map data");
    }
}

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

    let gat: LifGat = root_extension(gltf, lif::EXTENSION_GAT)?
        .ok_or_else(|| format!("missing root extension {}", lif::EXTENSION_GAT))?;
    let raw = gat_bytes(gltf, &gat)?;
    let altitude = LifGat::decode(raw)
        .map_err(|error| format!("decoding the {} bufferView: {error}", lif::EXTENSION_GAT))?;
    // `LifGat::validate` re-parses the whole grid; the check is two
    // comparisons and this runs on every map load.
    if altitude.width != gat.width || altitude.height != gat.height {
        return Err(format!(
            "{} declares {}x{} but its bufferView parses to {}x{}",
            lif::EXTENSION_GAT,
            gat.width,
            gat.height,
            altitude.width,
            altitude.height
        ));
    }

    let water = root_extension::<LifWater>(gltf, lif::EXTENSION_WATER)?.map(RswWater::from);

    Ok(LifMapData {
        altitude,
        water,
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

/// The `LIF_gat` bufferView, resolved against the GLB binary chunk. The
/// converter always embeds the GAT there, so a URI-backed buffer means the
/// file was rewritten by something that did not preserve the contract.
fn gat_bytes<'a>(gltf: &'a gltf::Gltf, gat: &LifGat) -> Result<&'a [u8], String> {
    let view = gltf.views().nth(gat.buffer_view as usize).ok_or_else(|| {
        format!(
            "{} points at missing bufferView {}",
            lif::EXTENSION_GAT,
            gat.buffer_view
        )
    })?;

    if !matches!(view.buffer().source(), gltf::buffer::Source::Bin) {
        return Err(format!(
            "{} bufferView {} is not backed by the GLB binary chunk",
            lif::EXTENSION_GAT,
            gat.buffer_view
        ));
    }

    let blob = gltf
        .blob
        .as_deref()
        .ok_or_else(|| "glb has no binary chunk to read the GAT from".to_string())?;
    let start = view.offset();
    let end = start + view.length();

    blob.get(start..end).ok_or_else(|| {
        format!(
            "{} bufferView {start}..{end} is outside the {}-byte binary chunk",
            lif::EXTENSION_GAT,
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
    use crate::infrastructure::assets::hierarchical_reader::HierarchicalAssetReader;
    use crate::infrastructure::assets::loaders::{RoAltitudeAsset, RoGroundAsset, RoWorldAsset};
    use crate::infrastructure::assets::sources::{CompositeAssetSource, DataFolderSource};
    use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::gltf::GltfPlugin;
    use bevy::image::{CompressedImageFormats, ImageLoader, ImagePlugin};
    use bevy::light::CascadeShadowConfig;
    use bevy::mesh::MeshPlugin;
    use bevy::world_serialization::{WorldAsset, WorldSerializationPlugin};
    use std::sync::{Arc, RwLock};
    use std::time::{Duration, Instant};

    /// The committed glbs are produced by
    /// `ro-to-lifthrasir-cli`'s ignored `regenerate_game_engine_fixtures` test.
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
            GltfMapPlugin,
        ));
        app.register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE));
        // `spawn_gltf_map` requires it, and requiring it is the point: without a
        // composite source there is no way to tell a glb map from a native one.
        app.insert_resource(SharedCompositeAssetSource(Arc::new(RwLock::new(
            CompositeAssetSource::new(),
        ))));
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

            assert_eq!(water.level, 12.5);
            assert_eq!(water.water_type, 3);
            assert_eq!(water.wave_height, 0.6);
            assert_eq!(water.wave_speed, 1.25);
            assert_eq!(water.wave_pitch, 45.0);
            assert_eq!(water.anim_speed, 4);
        });
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
                    model: "ro://data/model/prontera/tree01.rsm".to_string(),
                }
            );
        });
    }

    #[test]
    fn lights_get_the_native_shadow_tuning() {
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

    /// Lays a fixture glb out the way the converter ships it -- and the way
    /// [`spawn_gltf_map`] looks for it -- inside a throwaway `ro://` root.
    fn stage_map(map: &str, fixture: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let map_dir = root.path().join(format!("data/maps/{map}"));
        std::fs::create_dir_all(map_dir.join("tex")).unwrap();
        std::fs::copy(
            format!("{FIXTURES}/{fixture}"),
            map_dir.join(format!("{map}.glb")),
        )
        .unwrap();
        std::fs::copy(
            format!("{FIXTURES}/tex/grass01.png"),
            map_dir.join("tex/grass01.png"),
        )
        .unwrap();
        root
    }

    /// A headless app whose `ro://` source is `root`, running both the glb
    /// branch and the native extraction system it has to win against.
    fn map_app(root: &tempfile::TempDir) -> App {
        let mut composite = CompositeAssetSource::new();
        composite.add_source(Box::new(DataFolderSource::new(root.path())));
        let composite = Arc::new(RwLock::new(composite));

        let mut app = App::new();
        let reader_source = composite.clone();
        app.register_asset_source(
            AssetSourceId::Name("ro".into()),
            AssetSourceBuilder::new(move || {
                Box::new(HierarchicalAssetReader::new(reader_source.clone()))
            }),
        );
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
            MeshPlugin,
            WorldSerializationPlugin,
            GltfPlugin::default(),
            GltfMapPlugin,
        ));
        app.register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE));
        app.init_asset::<RoGroundAsset>()
            .init_asset::<RoAltitudeAsset>()
            .init_asset::<RoWorldAsset>();
        app.insert_resource(SharedCompositeAssetSource(composite));
        app.add_systems(Update, extract_map_from_unified_assets);
        app.finish();
        app.cleanup();
        app
    }

    fn request_map(app: &mut App, map: &str) -> Entity {
        app.world_mut()
            .spawn(MapRequestLoader::new(map.to_string()))
            .id()
    }

    /// Pumps the app until the glb scene has been instantiated under `entity`,
    /// plus one frame so the readiness observer has run.
    fn wait_for_scene(app: &mut App, entity: Entity) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while app.world().entity(entity).get::<Children>().is_none() {
            app.update();
            assert!(Instant::now() < deadline, "scene instance never spawned");
            std::thread::sleep(Duration::from_millis(2));
        }
        app.update();
    }

    #[test]
    fn a_map_with_a_glb_spawns_the_scene_instead_of_the_native_triple() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "mini_map.gat");

        app.update();

        let world = app.world();
        assert!(
            world.get::<MapLoader>(entity).is_none(),
            "the native .gnd/.gat/.rsw path must not run for a map with a glb"
        );
        assert!(world.get::<MapRequestLoader>(entity).unwrap().loaded);
        assert_eq!(
            world.get::<GltfMapLoader>(entity).unwrap().map_name,
            "mini_map"
        );
        assert!(world.get::<MapScoped>(entity).is_some());
        assert!(world.get::<WorldAssetRoot>(entity).is_some());
        assert_eq!(world.get::<Transform>(entity).unwrap().rotation, ROOT_FIX);
    }

    #[test]
    fn a_map_without_a_glb_falls_through_to_the_native_path() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "prontera.gat");

        app.update();

        let world = app.world();
        assert!(world.get::<GltfMapLoader>(entity).is_none());
        assert!(
            world.get::<MapLoader>(entity).is_some(),
            "a map with no glb must still load through the native path"
        );
    }

    #[test]
    fn a_healthy_glb_map_reaches_the_world_with_its_lif_data() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "mini_map");

        app.update();
        wait_for_scene(&mut app, entity);

        let world = app.world_mut();
        let scene_root = world
            .entity(entity)
            .get::<Children>()
            .expect("scene instance")[0];
        assert!(world.entity(scene_root).get::<LifMapData>().is_some());
        assert_eq!(
            world.query::<&LifMapLoadError>().iter(world).count(),
            0,
            "a healthy map must not report a load error"
        );
    }

    #[test]
    #[should_panic(expected = "map glb for 'bad_version' is unusable")]
    fn a_broken_glb_map_fails_loudly_instead_of_spawning_a_partial_map() {
        let root = stage_map("bad_version", "bad_version.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "bad_version");

        app.update();
        wait_for_scene(&mut app, entity);
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
