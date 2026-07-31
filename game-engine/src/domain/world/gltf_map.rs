//! Runtime side of the GLB map pipeline: the `LIF_*` glTF extension
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

use crate::domain::audio::map_sounds::spawn_gltf_map_sounds;
use crate::domain::effects::map_effects::spawn_gltf_map_effects;
use crate::domain::entities::pathfinding::{CurrentMapPathfindingGrid, PathfindingGrid};
use crate::domain::system_sets::WorldLoadingSystems;
use crate::domain::world::components::CurrentMapAltitude;
use crate::domain::world::gltf_prop::{
    PropAnim, play_prop_uv_animation, spawn_gltf_map_props, wire_pending_prop_scenes,
};
use crate::domain::world::map::MapData;
use crate::domain::world::map_loader::MapRequestLoader;
use crate::domain::world::map_scoped::MapScoped;
use crate::infrastructure::assets::loaders::RoAltitudeAsset;
use crate::presentation::rendering::water::begin_water_loading;
use bevy::app::AnimationSystems;
use bevy::asset::LoadContext;
use bevy::gltf::GltfAssetLabel;
use bevy::gltf::GltfLoaderSettings;
use bevy::gltf::extensions::{
    ErasedGltfExtensionHandler, GltfExtensionHandler, GltfExtensionHandlers,
};
use bevy::light::{CascadeShadowConfigBuilder, light_consts::lux};
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};
use lifthrasir_data::lif::{self, LifAudio, LifEffect, LifGat, LifMap, LifProp, LifWater};
use ro_formats::RoAltitude;

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
    pub water: Option<LifWater>,
    pub water_tiles: Vec<(usize, usize)>,
    pub meta: LifMap,
}

#[derive(Resource, Debug, Clone, Copy)]
pub(crate) struct CurrentMapNoShadeTint(pub [f32; 3]);

impl Default for CurrentMapNoShadeTint {
    fn default() -> Self {
        Self([1.0; 3])
    }
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

/// The glb scene of the map currently loaded, held so that tearing the map down
/// cannot unload it.
///
/// `MapScoped` reaps the map root on every warp -- including a warp back to the
/// map you are standing on. Without this handle that teardown drops the last
/// reference to the glb, and the reload that immediately follows restores it
/// unevenly: meshes come back as brand new assets, while the textures are
/// resurrected in place with no asset event at all. A render world that dropped
/// them on the way out gets its geometry back and never hears about the
/// textures again -- untextured terrain.
///
/// Loading any other map drops the entry, and with it the previous map's
/// assets: another glb map replaces it, a natively served one removes it.
#[derive(Resource)]
struct LoadedMapScene {
    map_name: String,
    scene: Handle<WorldAsset>,
}

/// Runtime plugin for the GLB map pipeline (unified per-map glb loading).
pub struct GltfMapPlugin;

impl Plugin for GltfMapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentMapNoShadeTint>()
            .register_type::<LifMapData>()
            .register_type::<LifMapLoadError>()
            .register_type::<LifAudioEmitter>()
            .register_type::<LifEffectEmitter>()
            .register_type::<LifPropRef>()
            .register_type::<PropAnim>()
            .register_type::<GltfMapLoader>();

        app.world_mut()
            .resource_mut::<GltfExtensionHandlers>()
            .0
            .write_blocking()
            .push(Box::new(LifExtensionHandler::default()));

        app.add_systems(
            Update,
            (
                spawn_gltf_map.in_set(WorldLoadingSystems::AssetExtraction),
                detect_gltf_map_load_failure.in_set(WorldLoadingSystems::AssetFailureDetection),
            ),
        );
        // Emitter nodes are positioned by the scene hierarchy, so their world
        // position only exists once transforms have propagated -- which happens
        // in `PostUpdate`, after the `SpawnScene` schedule that instantiates
        // them.
        app.add_systems(
            PostUpdate,
            (
                spawn_gltf_map_sounds,
                spawn_gltf_map_effects,
                spawn_gltf_map_props,
                wire_pending_prop_scenes,
            )
                .after(TransformSystems::Propagate),
        );
        app.add_systems(PostUpdate, play_prop_uv_animation.after(AnimationSystems));
        app.add_observer(adopt_gltf_map_scene);
    }
}

/// Claims every map request and loads its converted glb scene.
fn spawn_gltf_map(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    loaded: Option<Res<LoadedMapScene>>,
    mut requests: Query<(Entity, &mut MapRequestLoader), Without<GltfMapLoader>>,
) {
    for (entity, mut request) in requests.iter_mut() {
        if request.loaded {
            continue;
        }

        let map_name = request.map_name.trim_end_matches(".gat").to_lowercase();
        let path = format!("maps/{map_name}/{map_name}.glb");
        info!("Loading map '{map_name}' from {path}");

        let scene = match loaded.as_deref() {
            Some(current) if current.map_name == map_name => current.scene.clone(),
            _ => asset_server.load(GltfAssetLabel::Scene(0).from_asset(format!("ro://{path}"))),
        };

        commands.insert_resource(LoadedMapScene {
            map_name: map_name.clone(),
            scene: scene.clone(),
        });
        commands.entity(entity).insert((
            WorldAssetRoot(scene),
            Transform::from_rotation(ROOT_FIX),
            MapScoped,
            GltfMapLoader { map_name },
        ));

        request.loaded = true;
    }
}

fn detect_gltf_map_load_failure(
    asset_server: Res<AssetServer>,
    maps: Query<(&WorldAssetRoot, &GltfMapLoader)>,
) {
    for (root, map) in maps.iter() {
        if let bevy::asset::LoadState::Failed(error) = asset_server.load_state(&root.0) {
            let map_name = &map.map_name;
            panic!("failed to load map glb 'maps/{map_name}/{map_name}.glb': {error:?}");
        }
    }
}

/// Publishes the ready glb scene's walkability grid, heightfield, ambient
/// light, water surface, and `MapData` on the loader entity.
///
/// A glb map that imported badly is a hard failure: bevy's extension hooks
/// cannot fail a load, so a broken `LIF_*` payload still reports `Loaded` and
/// would otherwise spawn a map missing its altitude, water and metadata.
#[allow(clippy::too_many_arguments)]
fn adopt_gltf_map_scene(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut altitudes: ResMut<Assets<RoAltitudeAsset>>,
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

    let Some(data) = children
        .iter_descendants(ready.entity)
        .find_map(|entity| map_data.get(entity).ok())
    else {
        panic!("map glb for '{map_name}' carries no LIF_map data");
    };

    commands.insert_resource(CurrentMapNoShadeTint(data.meta.no_shade_tint));
    commands.insert_resource(CurrentMapPathfindingGrid(PathfindingGrid::from_gat(
        &data.altitude,
    )));
    commands.insert_resource(CurrentMapAltitude(altitudes.add(RoAltitudeAsset {
        altitude: data.altitude.clone(),
    })));
    commands.insert_resource(ambient_light(&data.meta));
    commands.entity(ready.entity).insert(MapData {
        name: map_name.clone(),
        width: data.altitude.width,
        height: data.altitude.height,
    });

    if let Some(water) = data.water.as_ref() {
        begin_water_loading(
            &mut commands,
            &asset_server,
            ready.entity,
            water,
            data.water_tiles.clone(),
        );
    }
}

/// Applies the RSW ambient colour as-is, kept dim so the sun and point lights
/// keep contrast.
fn ambient_light(meta: &LifMap) -> GlobalAmbientLight {
    GlobalAmbientLight {
        color: Color::srgb(
            meta.ambient_color[0],
            meta.ambient_color[1],
            meta.ambient_color[2],
        ),
        brightness: lux::OFFICE,
        affects_lightmapped_meshes: false,
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
    let raw = buffer_view_bytes(gltf, gat.buffer_view as usize, lif::EXTENSION_GAT)?;
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
    use crate::domain::audio::map_sounds::{MapSound, MapSoundSource, map_sound_path};
    use crate::domain::character::events::MapLoadCompleted;
    use crate::domain::character::map_loading::detect_map_load_complete;
    use crate::domain::effects::components::{ActiveEffect, MapAmbientVfx};
    use crate::domain::entities::pathfinding::PathfindingGrid;
    use crate::domain::input::resources::ForwardedCursorPosition;
    use crate::domain::input::terrain_raycast::{
        TerrainRaycastCache, update_terrain_raycast_cache,
    };
    use crate::domain::world::gltf_prop::MapModel;
    use crate::domain::world::loading_progress::{GLTF_MAP_STEPS, track_map_load_progress};
    use crate::domain::world::spawn_context::MapSpawnContext;
    use crate::infrastructure::assets::hierarchical_reader::HierarchicalAssetReader;
    use crate::infrastructure::assets::loaders::RoAltitudeAsset;
    use crate::infrastructure::assets::sources::{CompositeAssetSource, DataFolderSource};
    use crate::infrastructure::effect::{EffectCatalog, EffectDataAsset};
    use crate::presentation::rendering::water::WaterLoadingState;
    use crate::utils::coordinates::world_position_to_spawn_coords;
    use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::ecs::system::RunSystemOnce;
    use bevy::gltf::{GltfMaterial, GltfPlugin};
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

    /// Lays a fixture glb out the way the converter ships it -- and the way
    /// [`spawn_gltf_map`] looks for it -- inside a throwaway `ro://` root.
    fn stage_map(map: &str, fixture: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        stage_map_into(&root, map, fixture);
        root
    }

    /// Adds another map to an existing `ro://` root, for the tests that need
    /// two maps to warp between.
    fn stage_map_into(root: &tempfile::TempDir, map: &str, fixture: &str) {
        let map_dir = root.path().join(format!("maps/{map}"));
        std::fs::create_dir_all(map_dir.join("tex")).unwrap();
        std::fs::copy(
            format!("{FIXTURES}/{fixture}"),
            map_dir.join(format!("{map}.glb")),
        )
        .unwrap();
        std::fs::copy(
            format!("{FIXTURES}/tex/grass01.ktx2"),
            map_dir.join("tex/grass01.ktx2"),
        )
        .unwrap();
    }

    /// A headless app whose `ro://` source is `root`, running the GLB map path.
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
            // The emitter systems read `GlobalTransform`, which only exists
            // once this plugin has propagated the scene hierarchy.
            TransformPlugin,
            AssetPlugin::default(),
            ImagePlugin::default(),
            MeshPlugin,
            WorldSerializationPlugin,
            GltfPlugin::default(),
            GltfMapPlugin,
        ));
        app.world_mut()
            .resource_mut::<GltfExtensionHandlers>()
            .0
            .write_blocking()
            .push(Box::new(StandardMaterialHandler));
        app.register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE));
        app.init_asset::<RoAltitudeAsset>()
            // `PbrPlugin`'s job in the real app; the terrain materials the glb
            // carries have nowhere to land without it, and the scene spawner
            // refuses to clone a component it cannot reflect.
            .init_asset::<StandardMaterial>()
            .register_type::<MeshMaterial3d<StandardMaterial>>();
        // The completion flow the adapter feeds: `MapData` on the loader entity
        // is what turns a finished map into `MapLoadCompleted`.
        app.add_message::<MapLoadCompleted>();
        app.insert_resource(MapSpawnContext::new("mini_map".to_string(), 0, 0, 1));
        app.add_systems(Update, detect_map_load_complete);
        app.finish();
        app.cleanup();
        app
    }

    /// The `LifMapData` the handler put on the instantiated scene root.
    fn scene_map_data(app: &App, entity: Entity) -> LifMapData {
        let world = app.world();
        let scene_root = world
            .entity(entity)
            .get::<Children>()
            .expect("scene instance")[0];
        world
            .entity(scene_root)
            .get::<LifMapData>()
            .expect("LifMapData on the scene root")
            .clone()
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

    /// The slice of `PbrPlugin` this file cares about: the glTF material turned
    /// into a `StandardMaterial` sub-asset, and the `MeshMaterial3d` pointing a
    /// spawned primitive at it. `PbrPlugin` itself needs a render app, and
    /// without this the scene holds no material handle at all -- so the texture
    /// lifetime a warp exercises would be invisible here.
    #[derive(Clone, Default)]
    struct StandardMaterialHandler;

    impl GltfExtensionHandler for StandardMaterialHandler {
        fn dyn_clone(&self) -> Box<dyn ErasedGltfExtensionHandler> {
            Box::new(self.clone())
        }

        fn on_material(
            &mut self,
            load_context: &mut LoadContext<'_>,
            _gltf_material: &gltf::Material,
            _material: Handle<GltfMaterial>,
            material_asset: &GltfMaterial,
            material_label: &str,
        ) {
            load_context.add_labeled_asset(
                format!("{material_label}/std"),
                StandardMaterial {
                    base_color_texture: material_asset.base_color_texture.clone(),
                    ..default()
                },
            );
        }

        fn on_spawn_mesh_and_material(
            &mut self,
            load_context: &mut LoadContext<'_>,
            _primitive: &gltf::Primitive,
            _mesh: &gltf::Mesh,
            _material: &gltf::Material,
            entity: &mut EntityWorldMut,
            material_label: &str,
        ) {
            entity.insert(MeshMaterial3d(
                load_context.get_label_handle::<StandardMaterial>(format!("{material_label}/std")),
            ));
        }
    }

    /// Mimics `despawn_map_scoped`: a warp reaps every map-scoped entity, and
    /// the glb loader entity -- scene instance and all -- is one of them.
    fn warp_away(app: &mut App) {
        let world = app.world_mut();
        let scoped: Vec<Entity> = world
            .query_filtered::<Entity, With<MapScoped>>()
            .iter(world)
            .collect();
        for entity in scoped {
            world.entity_mut(entity).despawn();
        }
    }

    fn descendants(world: &World, root: Entity) -> Vec<Entity> {
        let mut found = Vec::new();
        let mut stack = vec![root];
        while let Some(entity) = stack.pop() {
            found.push(entity);
            if let Some(children) = world.entity(entity).get::<Children>() {
                stack.extend(children.iter());
            }
        }
        found
    }

    /// The terrain material of the instance rooted at `entity`, which is the
    /// only textured mesh the fixture carries.
    fn terrain_material(world: &World, entity: Entity, stage: &str) -> Handle<StandardMaterial> {
        descendants(world, entity)
            .into_iter()
            .find_map(|descendant| world.get::<MeshMaterial3d<StandardMaterial>>(descendant))
            .unwrap_or_else(|| panic!("{stage}: no terrain mesh under the map root"))
            .0
            .clone()
    }

    /// Terrain geometry is worthless without its texture: the material must
    /// still be a live asset, and so must the image it samples.
    fn assert_terrain_texture_is_live(app: &mut App, entity: Entity, stage: &str) {
        let material = terrain_material(app.world(), entity, stage);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let image = app
                .world()
                .resource::<Assets<StandardMaterial>>()
                .get(&material)
                .unwrap_or_else(|| panic!("{stage}: the terrain material asset is gone"))
                .base_color_texture
                .clone()
                .unwrap_or_else(|| panic!("{stage}: the terrain material has no base colour map"));

            let state = app.world().resource::<AssetServer>().load_state(&image);
            assert!(!state.is_failed(), "{stage}: terrain texture {state:?}");
            if state.is_loaded()
                && app
                    .world()
                    .resource::<Assets<Image>>()
                    .get(&image)
                    .is_some()
            {
                return;
            }

            assert!(
                Instant::now() < deadline,
                "{stage}: the terrain texture never became a live asset ({state:?})"
            );
            app.update();
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Every mesh asset the instance rooted at `entity` renders, sorted so two
    /// instances of the same map compare equal.
    fn terrain_meshes(world: &World, entity: Entity) -> Vec<AssetId<Mesh>> {
        let mut ids: Vec<AssetId<Mesh>> = descendants(world, entity)
            .into_iter()
            .filter_map(|descendant| world.get::<Mesh3d>(descendant))
            .map(|mesh| mesh.0.id())
            .collect();
        ids.sort();
        ids
    }

    /// Warping to the map you are already standing on tears the map root down
    /// and asks for the very same glb again. That teardown must not unload the
    /// map's assets, because the reload that follows restores them unevenly:
    /// the meshes come back as brand new assets, while the textures are
    /// resurrected in place without a single asset event. A render world that
    /// dropped them on the way out therefore gets its geometry back and never
    /// hears about the textures again.
    #[test]
    fn warping_to_the_same_map_keeps_the_map_assets_loaded() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);

        let first = request_map(&mut app, "mini_map");
        app.update();
        wait_for_scene(&mut app, first);
        assert_terrain_texture_is_live(&mut app, first, "first load");

        let meshes = terrain_meshes(app.world(), first);
        assert!(!meshes.is_empty(), "the fixture map has terrain geometry");

        warp_away(&mut app);
        for _ in 0..5 {
            app.update();
        }

        for mesh in &meshes {
            assert!(
                app.world().resource::<Assets<Mesh>>().contains(*mesh),
                "the warp unloaded the map's terrain mesh, so the whole glb has to be reloaded"
            );
        }

        let second = request_map(&mut app, "mini_map");
        app.update();
        wait_for_scene(&mut app, second);
        assert_terrain_texture_is_live(&mut app, second, "after warp");
        assert_eq!(
            terrain_meshes(app.world(), second),
            meshes,
            "the second instance must reuse the assets the first one loaded"
        );
    }

    /// The other half of holding the map's scene: it is held for the *current*
    /// map only, so leaving for another one still lets the old map go.
    #[test]
    fn warping_to_another_map_releases_the_previous_one() {
        let root = stage_map("mini_map", "mini_map.glb");
        stage_map_into(&root, "no_water", "no_water.glb");
        let mut app = map_app(&root);

        let first = request_map(&mut app, "mini_map");
        app.update();
        wait_for_scene(&mut app, first);
        let meshes = terrain_meshes(app.world(), first);

        warp_away(&mut app);
        let second = request_map(&mut app, "no_water");
        app.update();
        wait_for_scene(&mut app, second);
        for _ in 0..5 {
            app.update();
        }

        for mesh in &meshes {
            assert!(
                !app.world().resource::<Assets<Mesh>>().contains(*mesh),
                "the map we warped away from is still holding its assets"
            );
        }
    }

    #[test]
    fn a_map_request_spawns_its_glb_scene() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "mini_map.gat");

        app.update();

        let world = app.world();
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
    #[should_panic(expected = "maps/prontera/prontera.glb")]
    fn a_map_without_a_glb_panics_with_the_expected_path() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        request_map(&mut app, "prontera.gat");
        let deadline = Instant::now() + Duration::from_secs(10);

        loop {
            app.update();
            assert!(
                Instant::now() < deadline,
                "missing map glb did not fail before timeout"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
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
        let map_data = world.entity(scene_root).get::<LifMapData>().unwrap();
        assert_eq!(
            world.resource::<CurrentMapNoShadeTint>().0,
            map_data.meta.no_shade_tint
        );
        assert_eq!(
            world.query::<&LifMapLoadError>().iter(world).count(),
            0,
            "a healthy map must not report a load error"
        );
    }

    #[test]
    fn a_ready_glb_map_publishes_the_pathfinding_grid() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "mini_map");

        app.update();
        wait_for_scene(&mut app, entity);

        let expected = PathfindingGrid::from_gat(&scene_map_data(&app, entity).altitude);
        let grid = app
            .world()
            .resource::<CurrentMapPathfindingGrid>()
            .0
            .clone();

        assert_eq!(
            (grid.width(), grid.height()),
            (expected.width(), expected.height())
        );
        for y in 0..expected.height() as u16 {
            for x in 0..expected.width() as u16 {
                assert_eq!(
                    grid.is_walkable(x, y),
                    expected.is_walkable(x, y),
                    "walkability differs at ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn a_ready_glb_map_publishes_the_map_altitude() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "mini_map");

        app.update();
        wait_for_scene(&mut app, entity);

        let expected = scene_map_data(&app, entity).altitude;
        let handle = app.world().resource::<CurrentMapAltitude>().0.clone();
        let published = &app
            .world()
            .resource::<Assets<RoAltitudeAsset>>()
            .get(&handle)
            .expect("the published handle must resolve to the decoded altitude")
            .altitude;

        assert_eq!(
            (published.width, published.height),
            (expected.width, expected.height)
        );
        assert_eq!(published.cells.len(), expected.cells.len());
        assert_eq!(published.cells[0].height, [0.0, 1.0, 2.0, 3.0]);
        assert_eq!(published.cells[15].height, [15.0, 16.0, 17.0, 18.0]);
        assert!(!published.cells[7].cell_type.is_walkable());
    }

    /// Ground picking on the glb path: the ray marches the GAT heightfield
    /// `CurrentMapAltitude` publishes, and lands on the same grid cell
    /// `world_position_to_spawn_coords` gives the click handler -- with no
    /// `.gnd` anywhere in the app.
    #[test]
    fn a_ready_glb_map_supports_terrain_ground_picking() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "mini_map");

        app.update();
        wait_for_scene(&mut app, entity);

        app.init_resource::<TerrainRaycastCache>();
        app.insert_resource(ForwardedCursorPosition {
            position: Some(VIEWPORT * 0.5),
        });
        // World up is -Y, so a camera above the terrain looks along +Y.
        let transform = Transform::from_xyz(7.0, -100.0, 7.0).looking_to(Vec3::Y, Vec3::Z);
        app.world_mut().spawn((
            Camera3d::default(),
            headless_camera(),
            transform,
            GlobalTransform::from(transform),
        ));

        app.world_mut()
            .run_system_once(update_terrain_raycast_cache)
            .expect("raycast system runs");

        let cache = app.world().resource::<TerrainRaycastCache>();
        let world_position = cache.world_position.expect("the ray must hit the terrain");
        // Fixture GAT cell (1, 1) is `[5, 6, 7, 8]`; bilinear at 40% into the
        // cell is 6.6, and the GAT lookup drops the usual 1.5.
        assert!(
            (world_position.y - 5.1).abs() < 0.02,
            "{world_position} is not the fixture terrain height"
        );
        assert!(
            world_position.xz().abs_diff_eq(Vec2::splat(7.0), 0.05),
            "{world_position} drifted off the camera axis"
        );

        // 7.0 world units is 1.4 GAT cells, and the cache applies the -1 X /
        // +1 Y correction on top.
        assert_eq!(
            world_position_to_spawn_coords(world_position),
            (1, 1),
            "the picked position must convert with GAT-resolution cells"
        );
        let (cell_x, cell_y) = cache.cell_coords.expect("the ray must resolve a cell");
        assert_eq!((cell_x, cell_y), (0, 2));
        // Every fixture cell carries raw type 1, which is not walkable.
        assert!(!cache.is_walkable);

        // Click-to-move pathfinds on the GAT-resolution grid, so the picked
        // cell has to be addressable there.
        let grid = &app.world().resource::<CurrentMapPathfindingGrid>().0;
        assert!(
            u32::from(cell_x) < grid.width() && u32::from(cell_y) < grid.height(),
            "picked cell ({cell_x}, {cell_y}) is outside the {}x{} walkability grid",
            grid.width(),
            grid.height()
        );
    }

    /// Logical viewport size for [`headless_camera`].
    const VIEWPORT: Vec2 = Vec2::new(1600.0, 900.0);

    /// A camera with the fields `Camera::viewport_to_world` needs, which
    /// nothing computes without the render app.
    fn headless_camera() -> Camera {
        let viewport = bevy::camera::Viewport {
            physical_size: VIEWPORT.as_uvec2(),
            ..default()
        };
        let mut camera = Camera {
            viewport: Some(viewport.clone()),
            ..default()
        };
        camera.computed.target_info = Some(bevy::camera::RenderTargetInfo {
            physical_size: viewport.physical_size,
            scale_factor: 1.0,
        });
        let mut projection = Projection::default();
        projection.update(VIEWPORT.x, VIEWPORT.y);
        camera.computed.clip_from_view = projection.get_clip_from_view();
        camera
    }

    #[test]
    fn a_ready_glb_map_completes_the_map_load() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "mini_map.gat");

        app.update();
        wait_for_scene(&mut app, entity);

        let altitude = scene_map_data(&app, entity).altitude;
        let map_data = app
            .world()
            .entity(entity)
            .get::<MapData>()
            .expect("MapData on the map loader entity");
        assert_eq!(map_data.name, "mini_map");
        assert_eq!(
            (map_data.width, map_data.height),
            (altitude.width, altitude.height)
        );

        app.update();

        let completed: Vec<String> = app
            .world_mut()
            .resource_mut::<Messages<MapLoadCompleted>>()
            .drain()
            .map(|message| message.map_name)
            .collect();
        assert_eq!(completed, vec!["mini_map".to_string()]);
    }

    #[test]
    fn a_loading_glb_map_reports_progress_of_its_own() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "mini_map");

        app.update();

        let tracked = |app: &mut App| {
            app.world_mut()
                .run_system_once(track_map_load_progress)
                .expect("map load progress")
        };

        assert_eq!(
            tracked(&mut app).total,
            GLTF_MAP_STEPS,
            "a glb map load must be tracked by the glb steps, not the native ones"
        );

        wait_for_scene(&mut app, entity);

        let done = tracked(&mut app);
        assert_eq!(
            (done.done, done.total),
            (GLTF_MAP_STEPS, GLTF_MAP_STEPS),
            "an adapted glb map is a finished map load"
        );

        app.world_mut().entity_mut(entity).remove::<MapData>();
        assert_eq!(
            tracked(&mut app).done,
            GLTF_MAP_STEPS - 1,
            "the loaded glb and its textures must count before MapData lands"
        );
    }

    #[test]
    fn a_ready_glb_map_publishes_the_rsw_ambient_light() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "mini_map");

        app.update();
        wait_for_scene(&mut app, entity);

        let ambient = app.world().resource::<GlobalAmbientLight>();
        assert_eq!(ambient.color, Color::srgb(0.25, 0.3, 0.35));
        assert_eq!(ambient.brightness, lux::OFFICE);
        assert!(!ambient.affects_lightmapped_meshes);
    }

    #[test]
    fn a_ready_glb_map_queues_water_from_its_lif_water_params() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "mini_map");

        app.update();
        wait_for_scene(&mut app, entity);

        let water = app
            .world()
            .entity(entity)
            .get::<WaterLoadingState>()
            .expect("water queued on the map loader entity");

        assert_eq!(water.zones.len(), 1);
        let zone = &water.zones[0];
        assert_eq!(zone.water_level, 12.5);
        assert_eq!(zone.wave_height_param, 0.6);
        assert_eq!(zone.wave_speed, 1.25);
        assert_eq!(zone.wave_pitch, 45.0);
        assert_eq!(zone.animation_speed, 4.0);
        assert!(
            (zone.wave_height - 11.9).abs() < 1e-4,
            "{}",
            zone.wave_height
        );
        // The fixture's baked 2x2 mask selects these two tiles.
        assert_eq!(zone.water_tiles, vec![(1, 0), (0, 1)]);
    }

    #[test]
    fn a_ready_glb_map_spawns_its_audio_emitter_as_a_map_sound() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "mini_map");

        app.update();
        wait_for_scene(&mut app, entity);

        let world = app.world_mut();
        let mut query = world.query_filtered::<(&MapSoundSource, &Transform), With<MapSound>>();
        let sounds: Vec<_> = query.iter(world).collect();
        assert_eq!(sounds.len(), 1, "one MapSound per glb audio emitter");

        let (source, transform) = sounds[0];
        assert_eq!(
            source.handle_path,
            map_sound_path("effect\\water.wav"),
            "the glb path must end at the same asset path the RSW path builds"
        );
        assert_eq!(source.base_volume, 0.75);
        assert_eq!(source.range, 30.0);
        assert_eq!(source.cycle, Duration::from_secs_f32(4.5));
        // RSW [1, -2, 3] on a 2x2 ground, baked by the converter and put back
        // in world space by `ROOT_FIX`.
        assert!(
            transform
                .translation
                .abs_diff_eq(Vec3::new(11.0, -2.0, 13.0), 1e-4),
            "{}",
            transform.translation
        );
    }

    /// A catalog carrying only the `special` entries a test needs.
    fn effect_catalog(special: &str) -> EffectCatalog {
        let asset =
            ron::from_str::<EffectDataAsset>(&format!("(skills: {{}}, special: {{{special}}})"))
                .expect("test catalog RON");
        EffectCatalog::build(&asset.0).expect("test catalog builds")
    }

    #[test]
    fn a_glb_effect_emitter_spawns_at_its_node_world_position() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        app.insert_resource(effect_catalog(
            r#"12: (visuals: [Bespoke("smoke")], sound: None, placement: Ground, color: (1.0, 1.0, 1.0, 1.0), repeating: true)"#,
        ));
        let entity = request_map(&mut app, "mini_map");

        app.update();
        wait_for_scene(&mut app, entity);
        app.update();
        app.update();

        let world = app.world_mut();
        let mut query = world.query_filtered::<(&MapAmbientVfx, &Transform), With<MapScoped>>();
        let effects: Vec<_> = query.iter(world).collect();
        assert_eq!(
            effects.len(),
            1,
            "one effect per glb effect emitter, however many frames run"
        );

        let (vfx, transform) = effects[0];
        assert_eq!(vfx.key, "smoke");
        assert_eq!(vfx.emit_speed, 0.25);
        assert_eq!(vfx.params, [1.0, 2.0, 3.0, 4.0]);
        // RSW [4, -6, 8] on a 2x2 ground, baked by the converter and put back
        // in world space by `ROOT_FIX`.
        assert!(
            transform
                .translation
                .abs_diff_eq(Vec3::new(14.0, -6.0, 18.0), 1e-4),
            "{}",
            transform.translation
        );
    }

    /// Same as the RSW path: an id the `special` table does not carry is warned
    /// about and skipped, and nothing else about the map changes.
    #[test]
    fn an_unmapped_glb_effect_id_spawns_nothing() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        app.insert_resource(effect_catalog(
            r#"44: (visuals: [Bespoke("smoke")], sound: None, placement: Ground, color: (1.0, 1.0, 1.0, 1.0), repeating: true)"#,
        ));
        let entity = request_map(&mut app, "mini_map");

        app.update();
        wait_for_scene(&mut app, entity);
        app.update();

        let world = app.world_mut();
        assert_eq!(world.query::<&MapAmbientVfx>().iter(world).count(), 0);
        assert_eq!(world.query::<&ActiveEffect>().iter(world).count(), 0);
    }

    #[test]
    fn later_frames_do_not_respawn_the_glb_map_sound() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "mini_map");

        app.update();
        wait_for_scene(&mut app, entity);
        app.update();
        app.update();

        let world = app.world_mut();
        let count = world.query::<&MapSound>().iter(world).count();
        assert_eq!(count, 1, "the emitter must only spawn its sound once");
    }

    /// The converter bakes the whole RSW placement into the prop node, so the
    /// runtime attaches the converted scene there and applies nothing on top.
    #[test]
    fn a_glb_prop_scene_uses_the_baked_placement() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "mini_map");

        app.update();
        wait_for_scene(&mut app, entity);
        app.update();
        app.update();

        let world = app.world_mut();
        let mut query = world.query::<(
            &LifPropRef,
            &MapModel,
            &Transform,
            &GlobalTransform,
            &Children,
        )>();
        let props: Vec<_> = query.iter(world).collect();
        assert_eq!(
            props.len(),
            1,
            "one map model per glb prop node, however many frames run"
        );

        let (prop, model, transform, global, children) = props[0];
        assert_eq!(model.filename, prop.0.model);
        assert!(prop.0.model.ends_with(".glb"));
        assert_eq!(prop.0.anim_type, 1);
        assert_eq!(prop.0.anim_speed, 2.0);
        assert_eq!(children.len(), 1);
        let scene = world.entity(children[0]);
        assert!(scene.contains::<WorldAssetRoot>());
        let anim = scene.get::<PropAnim>().expect("PropAnim on prop scene");
        assert_eq!(anim.model, prop.0.model);
        assert_eq!(anim.anim_type, prop.0.anim_type);
        assert_eq!(anim.anim_speed, prop.0.anim_speed);

        // RSW position [7, -8, 9], rotation [10, 20, 30] and scale [1, 2, 3] on
        // a 2x2 ground, baked by the converter into the node's own transform.
        assert!(
            transform
                .translation
                .abs_diff_eq(Vec3::new(17.0, 8.0, -19.0), 1e-4),
            "{}",
            transform.translation
        );
        assert!(
            transform.scale.abs_diff_eq(Vec3::new(1.0, 2.0, 3.0), 1e-4),
            "{}",
            transform.scale
        );

        let (scale, rotation, translation) = global.to_scale_rotation_translation();
        assert!(
            translation.abs_diff_eq(Vec3::new(17.0, -8.0, 19.0), 1e-4),
            "{translation}"
        );
        assert!(scale.abs_diff_eq(Vec3::new(1.0, 2.0, 3.0), 1e-4), "{scale}");

        let expected = Quat::from_rotation_z(30f32.to_radians())
            * Quat::from_rotation_x(10f32.to_radians())
            * Quat::from_rotation_y(20f32.to_radians());
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            assert!(
                (rotation * axis).abs_diff_eq(expected * axis, 1e-4),
                "{rotation} is not the baked RSW rotation {expected}"
            );
        }
    }

    #[test]
    fn a_glb_map_without_lif_water_queues_no_water() {
        let root = stage_map("no_water", "no_water.glb");
        let mut app = map_app(&root);
        let entity = request_map(&mut app, "no_water");

        app.update();
        wait_for_scene(&mut app, entity);

        assert!(scene_map_data(&app, entity).water.is_none());
        assert!(
            app.world()
                .entity(entity)
                .get::<WaterLoadingState>()
                .is_none(),
            "a map with no LIF_water must not queue a water surface"
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
