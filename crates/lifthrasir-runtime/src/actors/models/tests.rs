use super::systems::{ModelScene, ModelSceneLink, SceneReady};
use super::*;
use bevy::{
    animation::{AnimatedBy, AnimationPlugin, AnimationTargetId, animated_field},
    gltf::Gltf,
    mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    world_serialization::WorldAssetRoot,
};
use lifthrasir_data::gr2::IDLE;
use std::time::Duration;

fn fixture(idle: bool) -> (App, Entity, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), AnimationPlugin))
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .init_asset::<SkinnedMeshInverseBindposes>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(100),
        ))
        .add_plugins((
            lifthrasir_assets::ContentAssetsPlugin,
            crate::RuntimeCorePlugin,
            crate::ActorRenderPlugin,
        ));
    let target = AnimationTargetId::from_name(&Name::new("bone"));
    let mut clip = AnimationClip::default();
    clip.add_curve_to_target(
        target,
        AnimatableCurve::new(
            animated_field!(Transform::translation),
            UnevenSampleAutoCurve::new([(0.0, Vec3::ZERO), (1.0, Vec3::X)]).unwrap(),
        ),
    );
    let clip = app
        .world_mut()
        .resource_mut::<Assets<AnimationClip>>()
        .add(clip);
    let gltf = app.world_mut().resource_mut::<Assets<Gltf>>().add(Gltf {
        scenes: vec![Handle::default()],
        named_scenes: default(),
        meshes: vec![],
        named_meshes: default(),
        materials: vec![],
        named_materials: default(),
        nodes: vec![],
        named_nodes: default(),
        skins: vec![],
        named_skins: default(),
        default_scene: None,
        animations: vec![clip.clone()],
        named_animations: if idle {
            [(IDLE.into(), clip)].into()
        } else {
            default()
        },
        source: None,
    });
    let model = "ro://models/3dmob/synthetic.glb";
    let actor = app
        .world_mut()
        .spawn((
            Gr2Actor {
                model: model.into(),
            },
            CharacterDirection {
                facing: crate::actors::Direction::West,
            },
        ))
        .id();
    let root = app
        .world_mut()
        .spawn((
            ModelScene {
                model: model.into(),
                gltf,
            },
            SceneReady,
            WorldAssetRoot(Handle::default()),
            Transform::default(),
            ChildOf(actor),
        ))
        .id();
    app.world_mut()
        .entity_mut(actor)
        .insert(ModelSceneLink(root));
    let player = app
        .world_mut()
        .spawn((AnimationPlayer::default(), ChildOf(root)))
        .id();
    let joint = app
        .world_mut()
        .spawn((
            Transform::default(),
            target,
            AnimatedBy(player),
            ChildOf(root),
        ))
        .id();
    let bindposes = app
        .world_mut()
        .resource_mut::<Assets<SkinnedMeshInverseBindposes>>()
        .add(SkinnedMeshInverseBindposes::from(vec![Mat4::IDENTITY]));
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    let mesh = app
        .world_mut()
        .spawn((
            Mesh3d(Handle::default()),
            MeshMaterial3d(material),
            SkinnedMesh {
                inverse_bindposes: bindposes,
                joints: vec![joint],
            },
            ChildOf(root),
        ))
        .id();
    (app, actor, mesh, joint)
}

#[test]
fn synthetic_skin_wires_and_animates_through_the_shared_plugin() {
    let (mut app, actor, mesh, joint) = fixture(true);
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world().get::<ActorVisualStatus>(actor),
        Some(&ActorVisualStatus::Ready)
    );
    assert_eq!(app.world().get::<ActorVisualRoot>(mesh).unwrap().0, actor);
    assert_eq!(
        app.world().get::<SkinnedMesh>(mesh).unwrap().joints,
        vec![joint]
    );
    assert!(app.world().get::<Transform>(joint).unwrap().translation.x > 0.0);
    let root = app.world().get::<ModelSceneLink>(actor).unwrap().0;
    let rotation = app.world().get::<Transform>(root).unwrap().rotation;
    assert!((rotation * Vec3::Z).abs_diff_eq(Vec3::NEG_X, 1e-5));
    app.world_mut().entity_mut(actor).remove::<Gr2Actor>();
    app.update();
    assert!(app.world().get_entity(mesh).is_err());
    assert!(app.world().get::<ActorVisualStatus>(actor).is_none());
}

#[test]
fn missing_idle_is_an_explicit_status_without_a_runtime_panic() {
    let (mut app, actor, _, _) = fixture(false);
    app.update();
    let Some(ActorVisualStatus::Failed(error)) = app.world().get::<ActorVisualStatus>(actor) else {
        panic!("invalid model must report Failed");
    };
    assert!(error.contains("synthetic.glb"));
    assert!(error.contains("has no idle animation"));
}
