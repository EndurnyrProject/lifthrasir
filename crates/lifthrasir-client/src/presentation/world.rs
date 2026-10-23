//! Client map selection, retained-scene policy and activation of neutral scene outcomes.

use bevy::prelude::*;
use lifthrasir_assets::loaders::RoAltitudeAsset;
use lifthrasir_runtime::world::water::begin_water_loading;
use lifthrasir_runtime::world::{
    LifMapData, LifMapLoadError, LifPropRef, MapSceneHandle, MapSceneRequest, MapSceneStatus,
    MapSceneSystems, PropSceneContext,
};
use {
    crate::domain::{
        camera::resources::CurrentMapCameraProfile,
        entities::pathfinding::CurrentMapPathfindingGrid,
        system_sets::WorldLoadingSystems,
        world::{
            components::CurrentMapAltitude, map::MapData, map_loader::MapRequestLoader,
            map_scoped::MapScoped,
        },
    },
    lifthrasir_runtime::world::terrain::PathfindingGrid,
};

/// The one map root selected by client flow, not by a global data query.
#[derive(Resource, Debug, Clone, Copy)]
pub struct SelectedMapScene(pub Entity);

#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component, Debug, Clone)]
pub struct GltfMapLoader {
    pub map_name: String,
}

/// A strong, path-qualified one-entry cache retained across same-map teardown.
#[derive(Resource)]
struct LoadedMapScene {
    handle: MapSceneHandle,
}

pub(crate) fn register_client_world(app: &mut App) {
    app.register_type::<GltfMapLoader>()
        .add_systems(
            Update,
            request_client_scene
                .in_set(WorldLoadingSystems::AssetExtraction)
                .before(MapSceneSystems::Requests),
        )
        .add_systems(
            Update,
            retain_selected_scene
                .after(MapSceneSystems::Requests)
                .before(MapSceneSystems::Readiness),
        )
        .add_systems(
            Update,
            activate_selected_scene
                .after(MapSceneSystems::Readiness)
                .in_set(WorldLoadingSystems::AssetFailureDetection),
        );
    app.add_observer(fail_selected_scene_immediately);
    register_client_prop_inputs(app);
}

fn fail_selected_scene_immediately(
    inserted: On<Insert<MapSceneStatus>>,
    selected: Option<Res<SelectedMapScene>>,
    scenes: Query<(&GltfMapLoader, &MapSceneStatus, Has<LifMapLoadError>)>,
) {
    if selected.is_none_or(|selected| selected.0 != inserted.entity) {
        return;
    }
    if let Ok((map, MapSceneStatus::Failed(error), decoded_error)) = scenes.get(inserted.entity) {
        fail_client_map(map, error, decoded_error);
    }
}

fn fail_client_map(map: &GltfMapLoader, error: &str, decoded_error: bool) -> ! {
    let name = &map.map_name;
    if decoded_error {
        panic!("map glb for '{name}' is unusable: {error}");
    }
    panic!("failed to load map glb 'maps/{name}/{name}.glb': {error}");
}

pub(crate) fn register_client_prop_inputs(app: &mut App) {
    app.add_systems(
        PostUpdate,
        supply_client_prop_context
            .after(bevy::transform::TransformSystems::Propagate)
            .before(lifthrasir_runtime::world::props::spawn_gltf_map_props),
    );
}

fn supply_client_prop_context(
    mut commands: Commands,
    selected: Option<Res<SelectedMapScene>>,
    maps: Query<&PropSceneContext, With<MapData>>,
    props: Query<Entity, (With<LifPropRef>, Without<PropSceneContext>)>,
) {
    let context = selected
        .and_then(|selected| maps.get(selected.0).ok())
        .copied()
        .unwrap_or_default();
    for entity in &props {
        commands.entity(entity).insert(context);
    }
}

fn request_client_scene(
    mut commands: Commands,
    selected: Option<Res<SelectedMapScene>>,
    retained: Option<Res<LoadedMapScene>>,
    mut requests: Query<&mut MapRequestLoader, Without<GltfMapLoader>>,
) {
    let Some(selected) = selected else {
        return;
    };
    let Ok(mut request) = requests.get_mut(selected.0) else {
        return;
    };
    if request.loaded {
        return;
    }
    let map_name = request.map_name.trim_end_matches(".gat").to_lowercase();
    let path = format!("ro://maps/{map_name}/{map_name}.glb");
    let mut entity = commands.entity(selected.0);
    if let Some(retained) = retained
        && retained.handle.path == path
    {
        entity.insert(retained.handle.clone());
    }
    entity.insert((
        MapSceneRequest { path },
        MapScoped,
        GltfMapLoader { map_name },
    ));
    request.loaded = true;
}

fn retain_selected_scene(
    mut commands: Commands,
    selected: Option<Res<SelectedMapScene>>,
    handles: Query<&MapSceneHandle, Changed<MapSceneHandle>>,
) {
    let Some(selected) = selected else {
        return;
    };
    if let Ok(handle) = handles.get(selected.0) {
        commands.insert_resource(LoadedMapScene {
            handle: handle.clone(),
        });
    }
}

type ActivationScenes<'w, 's> = Query<
    'w,
    's,
    (
        &'static GltfMapLoader,
        &'static MapSceneStatus,
        Option<&'static LifMapData>,
        Option<&'static LifMapLoadError>,
        Has<MapData>,
    ),
>;

pub(crate) fn activate_selected_scene(
    mut commands: Commands,
    selected: Option<Res<SelectedMapScene>>,
    scenes: ActivationScenes,
    asset_server: Res<AssetServer>,
    mut altitudes: ResMut<Assets<RoAltitudeAsset>>,
) {
    let Some(selected) = selected else {
        return;
    };
    let Ok((map, status, data, decode_error, activated)) = scenes.get(selected.0) else {
        return;
    };
    if let MapSceneStatus::Failed(error) = status {
        fail_client_map(map, error, decode_error.is_some());
    }
    if activated || *status != MapSceneStatus::Ready {
        return;
    }
    let data = data.expect("ready scene must carry decoded map data");
    commands.insert_resource(CurrentMapPathfindingGrid(PathfindingGrid::from_gat(
        &data.altitude,
    )));
    commands.insert_resource(CurrentMapAltitude(altitudes.add(RoAltitudeAsset {
        altitude: data.altitude.clone(),
    })));
    commands.insert_resource(lifthrasir_runtime::world::lighting::map_ambient_light(
        &data.meta,
    ));
    commands.insert_resource(CurrentMapCameraProfile {
        indoor: data.meta.indoor,
        exposure_ev100: data.meta.exposure_ev100,
    });
    commands.entity(selected.0).insert(MapData {
        name: map.map_name.clone(),
        width: data.altitude.width,
        height: data.altitude.height,
    });
    if let Some(water) = data.water.as_ref() {
        begin_water_loading(
            &mut commands,
            &asset_server,
            selected.0,
            water,
            data.water_tiles.clone(),
        );
    }
}
