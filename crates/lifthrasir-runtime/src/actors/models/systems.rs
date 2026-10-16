use super::{animation, *};
use bevy::{
    asset::{LoadState, RecursiveDependencyLoadState},
    camera::visibility::NoFrustumCulling,
    gltf::{Gltf, GltfAssetLabel, GltfMaterialName},
    world_serialization::{WorldAssetRoot, WorldInstanceReady},
};
use lifthrasir_data::gr2::EMBLEM_MATERIAL;

#[derive(Component)]
pub struct ModelScene {
    pub model: String,
    pub gltf: Handle<Gltf>,
}

#[derive(Component)]
pub struct ModelSceneLink(pub Entity);

#[derive(Component)]
pub struct SceneReady;

#[derive(Component)]
pub struct EmblemSurface {
    pub actor: Entity,
    pub default_texture: Option<Handle<Image>>,
}

pub fn sync_model_scenes(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    actors: Query<(Entity, &Gr2Actor, Option<&ModelSceneLink>)>,
    scenes: Query<(Entity, &ModelScene, &ChildOf)>,
) {
    for (entity, _, parent) in &scenes {
        if actors.get(parent.parent()).is_err() {
            commands.entity(entity).despawn();
            if let Ok(mut parent) = commands.get_entity(parent.parent()) {
                parent.remove::<(ModelSceneLink, ActorVisualStatus)>();
            }
        }
    }
    for (actor, model, link) in &actors {
        if let Some(link) = link
            && let Ok((root, scene, _)) = scenes.get(link.0)
        {
            if scene.model == model.model {
                continue;
            }
            commands.entity(root).despawn();
        }
        let root = commands
            .spawn((
                Name::new("GR2 model scene"),
                Transform::IDENTITY,
                WorldAssetRoot(
                    asset_server.load(GltfAssetLabel::Scene(0).from_asset(model.model.clone())),
                ),
                ModelScene {
                    model: model.model.clone(),
                    gltf: asset_server.load(&model.model),
                },
                ChildOf(actor),
            ))
            .observe(scene_ready)
            .id();
        commands
            .entity(actor)
            .insert((ModelSceneLink(root), ActorVisualStatus::Loading));
    }
}

fn scene_ready(ready: On<WorldInstanceReady>, mut commands: Commands) {
    commands.entity(ready.entity).insert(SceneReady);
}

pub(super) fn detect_load_failure(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    scenes: Query<(Entity, &ModelScene, &WorldAssetRoot, &ChildOf)>,
) {
    for (entity, scene, root, parent) in &scenes {
        let error = [scene.gltf.id().untyped(), root.0.id().untyped()]
            .into_iter()
            .find_map(|id| {
                if let LoadState::Failed(error) = asset_server.load_state(id) {
                    return Some(format!(
                        "failed to load GR2 model glb '{}': {error}",
                        scene.model
                    ));
                }
                if let Some(RecursiveDependencyLoadState::Failed(error)) =
                    asset_server.get_recursive_dependency_load_state(id)
                {
                    return Some(format!(
                        "failed to load GR2 model dependencies '{}': {error}",
                        scene.model
                    ));
                }
                None
            })
            .or_else(|| {
                gltfs
                    .get(&scene.gltf)
                    .filter(|gltf| gltf.scenes.is_empty())
                    .map(|_| format!("GR2 model '{}' has no scene 0", scene.model))
            });
        if let Some(error) = error {
            fail_scene(&mut commands, parent.parent(), entity, error);
        }
    }
}

fn fail_scene(commands: &mut Commands, actor: Entity, scene: Entity, error: String) {
    commands
        .entity(actor)
        .insert(ActorVisualStatus::Failed(error));
    commands.entity(scene).remove::<SceneReady>();
}

type SceneMeshes<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static MeshMaterial3d<StandardMaterial>>,
        Option<&'static GltfMaterialName>,
    ),
    With<Mesh3d>,
>;

/// Scene spawning can precede material insertion. Wire only when the actual assets exist.
#[allow(clippy::too_many_arguments)]
pub fn wire_scenes(
    mut commands: Commands,
    pending: Query<(Entity, &ModelScene, &ChildOf), With<SceneReady>>,
    children: Query<&Children>,
    gltfs: Res<Assets<Gltf>>,
    clips: Res<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    meshes: SceneMeshes,
    mut materials: ResMut<Assets<StandardMaterial>>,
    players: Query<(), With<AnimationPlayer>>,
    emblems: Query<(), With<ActorEmblem>>,
    statuses: Query<&ActorVisualStatus>,
) {
    for (root, scene, parent) in &pending {
        let actor = parent.parent();
        if matches!(statuses.get(actor), Ok(ActorVisualStatus::Failed(_))) {
            continue;
        }
        let Some(gltf) = gltfs.get(&scene.gltf) else {
            continue;
        };
        let descendants: Vec<_> = children.iter_descendants(root).collect();
        let mesh_entities: Vec<_> = descendants
            .iter()
            .copied()
            .filter(|e| meshes.contains(*e))
            .collect();
        if mesh_entities.is_empty() {
            fail_scene(
                &mut commands,
                actor,
                root,
                format!("GR2 model '{}' has no meshes", scene.model),
            );
            continue;
        }
        if mesh_entities.iter().any(|&e| {
            meshes
                .get(e)
                .is_ok_and(|(handle, _)| handle.is_some_and(|h| !materials.contains(&h.0)))
        }) {
            continue;
        }
        if emblems.contains(actor)
            && !mesh_entities.iter().any(|&e| {
                meshes
                    .get(e)
                    .is_ok_and(|(_, name)| name.is_some_and(|n| n.0 == EMBLEM_MATERIAL))
            })
        {
            fail_scene(
                &mut commands,
                actor,
                root,
                format!("GR2 flag '{}' has no emblem material", scene.model),
            );
            continue;
        }
        if mesh_entities
            .iter()
            .any(|&e| meshes.get(e).is_ok_and(|(handle, _)| handle.is_none()))
        {
            fail_scene(
                &mut commands,
                actor,
                root,
                format!("GR2 model '{}' has no standard material", scene.model),
            );
            continue;
        }
        let player_entities: Vec<_> = descendants
            .iter()
            .copied()
            .filter(|e| players.contains(*e))
            .collect();
        if player_entities.is_empty() {
            fail_scene(
                &mut commands,
                actor,
                root,
                format!("GR2 model '{}' has no animation player", scene.model),
            );
            continue;
        }
        let (playback, graph) =
            match animation::playback(actor, &scene.model, gltf, &clips, &mut graphs) {
                Ok(Some(playback)) => playback,
                Ok(None) => continue,
                Err(error) => {
                    fail_scene(&mut commands, actor, root, error);
                    continue;
                }
            };
        for player in player_entities {
            commands
                .entity(player)
                .insert((playback.clone(), graph.clone()));
        }
        for mesh in mesh_entities {
            let (handle, name) = meshes.get(mesh).expect("mesh checked above");
            let handle = handle.expect("standard material validated above");
            commands.entity(mesh).insert((
                ActorVisualRoot(actor),
                Pickable::default(),
                NoFrustumCulling,
            ));
            if name.is_some_and(|n| n.0 == EMBLEM_MATERIAL) {
                let material = materials.get(&handle.0).expect("material ready").clone();
                let default_texture = material.base_color_texture.clone();
                commands.entity(mesh).insert((
                    MeshMaterial3d(materials.add(material)),
                    EmblemSurface {
                        actor,
                        default_texture,
                    },
                ));
            }
        }
        commands.entity(root).remove::<SceneReady>();
        commands.entity(actor).insert(ActorVisualStatus::Ready);
    }
}

/// South is +Z in the engine and the converted source model faces +Z, so
/// direction 0 (South) is the identity. RO direction indices step clockwise as
/// seen from above, which is a negative yaw about +Y.
pub fn facing_rotation(direction: crate::rendering::Direction) -> Quat {
    Quat::from_rotation_y(-(direction as u8 as f32) * std::f32::consts::FRAC_PI_4)
}

pub(super) fn sync_facing(
    actors: Query<&CharacterDirection, With<Gr2Actor>>,
    mut scenes: Query<(&ChildOf, &mut Transform), With<ModelScene>>,
) {
    for (parent, mut transform) in &mut scenes {
        if let Ok(direction) = actors.get(parent.parent()) {
            let rotation = facing_rotation(direction.facing);
            if transform.rotation != rotation {
                transform.rotation = rotation;
            }
        }
    }
}

pub fn sync_emblems(
    surfaces: Query<(&EmblemSurface, &MeshMaterial3d<StandardMaterial>)>,
    owners: Query<&ActorEmblem>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (surface, handle) in &surfaces {
        let texture = owners
            .get(surface.actor)
            .ok()
            .and_then(|emblem| emblem.0.clone())
            .or_else(|| surface.default_texture.clone());
        if materials
            .get(&handle.0)
            .is_some_and(|m| m.base_color_texture != texture)
            && let Some(mut material) = materials.get_mut(&handle.0)
        {
            material.base_color_texture = texture;
        }
    }
}
