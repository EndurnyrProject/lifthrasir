//! Client composition around neutral GLB decoding/loading and selected-map activation.

pub use crate::presentation::world::GltfMapLoader;
use bevy::prelude::*;

pub struct GltfMapPlugin;
impl Plugin for GltfMapPlugin {
    fn build(&self, app: &mut App) {
        crate::presentation::world::register_client_world(app);
    }
}

#[cfg(test)]
use crate::{
    domain::{
        entities::pathfinding::CurrentMapPathfindingGrid,
        world::{
            components::CurrentMapAltitude, map::MapData, map_loader::MapRequestLoader,
            map_scoped::MapScoped,
        },
    },
    presentation::world::SelectedMapScene,
};
#[cfg(test)]
use bevy::world_serialization::WorldAssetRoot;
#[cfg(test)]
use lifthrasir_runtime::world::{MapSceneRequest, MapSceneStatus, PropSceneContext};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::character::events::MapLoadCompleted;
    use crate::domain::character::map_loading::detect_map_load_complete;
    use crate::domain::input::resources::ForwardedCursorPosition;
    use crate::domain::input::terrain_raycast::{
        TerrainRaycastCache, update_terrain_raycast_cache,
    };
    use crate::domain::world::loading_progress::{GLTF_MAP_STEPS, track_map_load_progress};
    use crate::domain::world::spawn_context::MapSpawnContext;
    use bevy::asset::LoadContext;
    use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::ecs::system::RunSystemOnce;
    use bevy::gltf::extensions::{
        ErasedGltfExtensionHandler, GltfExtensionHandler, GltfExtensionHandlers,
    };
    use bevy::gltf::{GltfMaterial, GltfPlugin};
    use bevy::image::{CompressedImageFormats, ImageLoader, ImagePlugin};
    use bevy::mesh::MeshPlugin;
    use bevy::world_serialization::WorldSerializationPlugin;
    use lifthrasir_assets::catalogs::effect::{EffectCatalog, EffectDataAsset};
    use lifthrasir_assets::loaders::RoAltitudeAsset;
    use lifthrasir_assets::source::hierarchical_reader::HierarchicalAssetReader;
    use lifthrasir_assets::source::{
        composite::CompositeAssetSource, data_folder::DataFolderSource,
    };
    use lifthrasir_runtime::audio::map_sounds::{MapSound, MapSoundSource, map_sound_path};
    use lifthrasir_runtime::effects::{ActiveEffect, MapAmbientVfx};
    use lifthrasir_runtime::rendering::coordinates::world_position_to_spawn_coords;
    use lifthrasir_runtime::world::props::{MapModel, PropAnim};
    use lifthrasir_runtime::world::terrain::PathfindingGrid;
    use lifthrasir_runtime::world::water::WaterLoadingState;
    use lifthrasir_runtime::world::{LifMapData, LifMapLoadError, LifPropRef};
    use std::sync::{Arc, RwLock};
    use std::time::{Duration, Instant};

    /// The committed glbs are produced by
    /// `ro-to-lifthrasir-cli`'s ignored `regenerate_game_engine_fixtures` test.
    const FIXTURES: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../lifthrasir-runtime/tests/fixtures"
    );

    /// Lays a fixture glb out the way the converter ships it -- and the way
    /// [`lifthrasir_runtime::world::MapScenePlugin`] looks for it -- inside a throwaway `ro://` root.
    fn stage_map(map: &str, fixture: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        stage_map_into(&root, map, fixture);
        root
    }

    /// Adds another map to an existing `ro://` root, for the tests that need
    /// two maps to warp between.
    fn stage_map_into(root: &tempfile::TempDir, map: &str, fixture: &str) {
        let map_dir = root.path().join(format!("maps/{map}"));
        std::fs::create_dir_all(&map_dir).unwrap();
        std::fs::create_dir_all(root.path().join("ground/tex")).unwrap();
        std::fs::copy(
            format!("{FIXTURES}/{fixture}"),
            map_dir.join(format!("{map}.glb")),
        )
        .unwrap();
        std::fs::copy(
            format!("{FIXTURES}/ground/tex/grass01.ktx2"),
            root.path().join("ground/tex/grass01.ktx2"),
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
            lifthrasir_runtime::WorldRenderPlugin,
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
        let entity = app
            .world_mut()
            .spawn(MapRequestLoader::new(map.to_string()))
            .id();
        app.insert_resource(SelectedMapScene(entity));
        entity
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

            let asset_path = app
                .world()
                .resource::<AssetServer>()
                .get_path(image.id())
                .unwrap_or_else(|| panic!("{stage}: the terrain texture has no asset path"));
            assert_eq!(
                asset_path.to_string(),
                "ro://ground/tex/grass01.ktx2",
                "{stage}: the terrain texture must resolve to the pooled ground URI"
            );

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
        assert_eq!(
            world.get::<Transform>(entity).unwrap().rotation,
            Quat::IDENTITY,
            "the glb is already in world space; no root rotation is applied"
        );
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
            world
                .get::<PropSceneContext>(entity)
                .expect("scene-derived tint context")
                .no_shade_tint,
            map_data.meta.no_shade_tint
        );
        assert_eq!(
            world.query::<&LifMapLoadError>().iter(world).count(),
            0,
            "a healthy map must not report a load error"
        );
    }

    #[test]
    fn an_unselected_ready_root_does_not_activate_client_map_resources() {
        let root = stage_map("mini_map", "mini_map.glb");
        let mut app = map_app(&root);
        let selected = request_map(&mut app, "mini_map");
        app.update();
        wait_for_scene(&mut app, selected);
        let altitude = app.world().resource::<CurrentMapAltitude>().0.clone();
        let other = app
            .world_mut()
            .spawn((
                MapSceneRequest {
                    path: "ro://maps/mini_map/mini_map.glb".into(),
                },
                GltfMapLoader {
                    map_name: "unselected".into(),
                },
            ))
            .id();
        app.update();
        wait_for_scene(&mut app, other);
        assert_eq!(
            app.world().get::<MapSceneStatus>(other),
            Some(&MapSceneStatus::Ready)
        );
        assert!(app.world().get::<MapData>(other).is_none());
        assert_eq!(app.world().resource::<CurrentMapAltitude>().0, altitude);
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
        // A camera above the terrain looks straight down; cell +y is -Z.
        let transform = Transform::from_xyz(7.0, 100.0, -7.0).looking_to(Vec3::NEG_Y, Vec3::NEG_Z);
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
        // cell is 6.6 (positive-down), and the GAT lookup lifts by 1.5.
        assert!(
            (world_position.y + 5.1).abs() < 0.02,
            "{world_position} is not the fixture terrain height"
        );
        assert!(
            world_position.xz().abs_diff_eq(Vec2::new(7.0, -7.0), 0.05),
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
        // mini_map is outdoor, so it carries the baked outdoor ambient floor.
        assert_eq!(ambient.brightness, 2_500.0);
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
        // RSW [1, -2, 3] on a 2x2 ground, baked by the converter into the
        // Y-up, north = -Z world.
        assert!(
            transform
                .translation
                .abs_diff_eq(Vec3::new(11.0, 2.0, -13.0), 1e-4),
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
        let mut query = world.query_filtered::<(&MapAmbientVfx, &Transform), With<lifthrasir_runtime::world::SceneOwner>>();
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
        // RSW [4, -6, 8] on a 2x2 ground, baked by the converter into the
        // Y-up, north = -Z world.
        assert!(
            transform
                .translation
                .abs_diff_eq(Vec3::new(14.0, 6.0, -18.0), 1e-4),
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
            translation.abs_diff_eq(Vec3::new(17.0, 8.0, -19.0), 1e-4),
            "{translation}"
        );
        assert!(scale.abs_diff_eq(Vec3::new(1.0, 2.0, 3.0), 1e-4), "{scale}");

        // The RSW rotation in RO's native (-Y-up) frame, conjugated by the
        // converter's 180-degree X rotation so the node places the Y-up prop
        // glb under it directly.
        let fix = Quat::from_rotation_x(std::f32::consts::PI);
        let expected = fix
            * Quat::from_rotation_z(30f32.to_radians())
            * Quat::from_rotation_x(10f32.to_radians())
            * Quat::from_rotation_y(20f32.to_radians())
            * fix;
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
}
