//! Explicit scene requests and per-root outcomes, without current-map policy.

use super::components::*;
use bevy::{
    asset::{LoadState, RecursiveDependencyLoadState},
    gltf::GltfAssetLabel,
    prelude::*,
    world_serialization::{WorldAssetRoot, WorldInstanceReady},
};

/// Ordering hooks for client request publication and activation after shared readiness.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MapSceneSystems {
    Requests,
    Readiness,
}

#[derive(Component)]
struct SceneAdopted;

/// Loads maps on caller-owned roots. Requires Bevy glTF/scene infrastructure and MapGltfPlugin.
pub struct MapScenePlugin;
impl Plugin for MapScenePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (MapSceneSystems::Requests, MapSceneSystems::Readiness).chain(),
        )
        .add_systems(Update, start_requests.in_set(MapSceneSystems::Requests))
        .add_systems(Update, update_readiness.in_set(MapSceneSystems::Readiness))
        .add_observer(adopt_scene_data);
    }
}

type Requests<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static MapSceneRequest,
        Option<&'static MapSceneHandle>,
        Option<&'static WorldAssetRoot>,
    ),
    Changed<MapSceneRequest>,
>;

fn start_requests(mut commands: Commands, assets: Res<AssetServer>, requests: Requests) {
    for (entity, request, retained, current) in &requests {
        if !request.path.starts_with("ro://") || !request.path.ends_with(".glb") {
            fail(
                &mut commands,
                entity,
                &request.path,
                "expected an explicit ro://...glb path".into(),
            );
            continue;
        }
        if let Some(retained) = retained
            && retained.path != request.path
        {
            fail(
                &mut commands,
                entity,
                &request.path,
                format!("retained scene belongs to '{}'", retained.path),
            );
            continue;
        }
        if retained.is_some_and(|handle| current.is_some_and(|root| root.0 == handle.scene)) {
            continue;
        }
        commands
            .entity(entity)
            .remove::<(LifMapData, LifMapLoadError, PropSceneContext, SceneAdopted)>();
        let scene = retained.map_or_else(
            || assets.load(GltfAssetLabel::Scene(0).from_asset(request.path.clone())),
            |handle| handle.scene.clone(),
        );
        commands.entity(entity).insert((
            MapSceneHandle {
                path: request.path.clone(),
                scene: scene.clone(),
            },
            WorldAssetRoot(scene),
            MapSceneStatus::Loading,
        ));
    }
}

fn fail(commands: &mut Commands, entity: Entity, path: &str, message: String) {
    commands
        .entity(entity)
        .remove::<(LifMapData, PropSceneContext, SceneAdopted)>()
        .insert(MapSceneStatus::Failed(format!(
            "map scene '{path}': {message}"
        )));
}

fn adopt_scene_data(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    requests: Query<(&MapSceneRequest, &MapSceneStatus)>,
    children: Query<&Children>,
    data: Query<&LifMapData>,
    errors: Query<&LifMapLoadError>,
    props: Query<(), With<LifPropRef>>,
) {
    let Ok((request, status)) = requests.get(ready.entity) else {
        return;
    };
    if matches!(status, MapSceneStatus::Failed(_)) {
        return;
    }
    if let Some(error) = children
        .iter_descendants(ready.entity)
        .find_map(|entity| errors.get(entity).ok())
    {
        commands.entity(ready.entity).insert(error.clone());
        fail(&mut commands, ready.entity, &request.path, error.0.clone());
        return;
    }
    let Some(data) = children
        .iter_descendants(ready.entity)
        .find_map(|entity| data.get(entity).ok())
    else {
        let message = "carries no LIF_map data".to_string();
        commands
            .entity(ready.entity)
            .insert(LifMapLoadError(message.clone()));
        fail(&mut commands, ready.entity, &request.path, message);
        return;
    };
    let context = PropSceneContext {
        no_shade_tint: data.meta.no_shade_tint,
    };
    for descendant in children.iter_descendants(ready.entity) {
        let mut entity = commands.entity(descendant);
        entity.insert(SceneOwner(ready.entity));
        if props.contains(descendant) {
            entity.insert(context);
        }
    }
    commands
        .entity(ready.entity)
        .insert((data.clone(), context, SceneAdopted));
}

fn update_readiness(
    mut commands: Commands,
    assets: Res<AssetServer>,
    roots: Query<
        (Entity, &MapSceneHandle, &MapSceneStatus, Has<SceneAdopted>),
        With<MapSceneRequest>,
    >,
) {
    for (entity, handle, status, adopted) in &roots {
        if matches!(status, MapSceneStatus::Failed(_)) {
            continue;
        }
        let error = match assets.load_state(&handle.scene) {
            LoadState::Failed(error) => Some(error.to_string()),
            _ => match assets.get_recursive_dependency_load_state(&handle.scene) {
                Some(RecursiveDependencyLoadState::Failed(error)) => Some(error.to_string()),
                _ => None,
            },
        };
        if let Some(error) = error {
            fail(&mut commands, entity, &handle.path, error);
        } else if adopted
            && *status != MapSceneStatus::Ready
            && assets.is_loaded_with_dependencies(&handle.scene)
        {
            commands.entity(entity).insert(MapSceneStatus::Ready);
        }
    }
}
