use bevy::{
    asset::io::AssetSourceBuilder, gltf::GltfPlugin, mesh::MeshPlugin, prelude::*,
    world_serialization::WorldSerializationPlugin,
};
use lifthrasir_runtime::world::{
    LifMapData, MapEmittersPlugin, MapGltfPlugin, MapSceneHandle, MapScenePlugin, MapSceneRequest,
    MapSceneStatus,
};
use std::time::{Duration, Instant};

fn app() -> App {
    let mut app = App::new();
    let fixtures = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
    app.register_asset_source("ro", AssetSourceBuilder::platform_default(fixtures, None));
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        ImagePlugin::default(),
        MeshPlugin,
        WorldSerializationPlugin,
        GltfPlugin::default(),
        MapGltfPlugin,
        MapScenePlugin,
        MapEmittersPlugin,
    ))
    .init_asset::<StandardMaterial>();
    app.finish();
    app.cleanup();
    app
}

fn settle(app: &mut App, root: Entity) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        if matches!(
            app.world().get::<MapSceneStatus>(root),
            Some(MapSceneStatus::Ready | MapSceneStatus::Failed(_))
        ) {
            return;
        }
        assert!(Instant::now() < deadline, "scene request never settled");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn scene_emitters_are_removed_only_with_their_owner_and_keep_world_space() {
    use lifthrasir_data::{EffectData, EffectDescriptor, EffectPlacement, GroundAnchor, Visual};
    use lifthrasir_runtime::world::{LifAudioEmitter, SceneOwner};
    use lifthrasir_runtime::{
        audio::{
            ContentAudioListener,
            map_sounds::{MapSound, MapSoundSource},
        },
        effects::MapAmbientVfx,
    };
    let mut app = app();
    let mut catalog = EffectData::default();
    catalog.special.insert(
        12,
        EffectDescriptor {
            visuals: vec![Visual::Bespoke("smoke".into())],
            sound: None,
            on_trigger: None,
            placement: EffectPlacement::Ground,
            color: [1.0; 4],
            repeating: true,
            ground_anchor: GroundAnchor::Group,
        },
    );
    app.insert_resource(lifthrasir_assets::catalogs::EffectCatalog::build(&catalog).unwrap());
    let listener = app
        .world_mut()
        .spawn((ContentAudioListener, bevy_kira_audio::SpatialAudioReceiver))
        .id();
    let first = app
        .world_mut()
        .spawn((
            MapSceneRequest {
                path: "ro://mini_map.glb".into(),
            },
            Transform::from_xyz(100.0, 0.0, 0.0),
        ))
        .id();
    let second = app
        .world_mut()
        .spawn(MapSceneRequest {
            path: "ro://mini_map.glb".into(),
        })
        .id();
    settle(&mut app, first);
    settle(&mut app, second);
    app.update();
    let emitter_position = app
        .world_mut()
        .query::<(&LifAudioEmitter, &GlobalTransform, &SceneOwner)>()
        .iter(app.world())
        .find(|(_, _, owner)| owner.0 == first)
        .unwrap()
        .1
        .translation();
    let (sound, _, position, _) = app
        .world_mut()
        .query::<(Entity, &MapSoundSource, &Transform, &SceneOwner)>()
        .iter(app.world())
        .find(|(_, _, _, owner)| owner.0 == first)
        .unwrap();
    assert_eq!(position.translation, emitter_position);
    assert!(app.world().get::<ChildOf>(sound).is_none());
    let count = |app: &mut App, root| {
        app.world_mut()
            .query::<(&SceneOwner, Has<MapSound>, Has<MapAmbientVfx>)>()
            .iter(app.world())
            .filter(|(owner, sound, effect)| owner.0 == root && (*sound || *effect))
            .count()
    };
    assert_eq!(count(&mut app, first), 2);
    assert_eq!(count(&mut app, second), 2);
    app.world_mut().entity_mut(first).despawn();
    app.update();
    assert_eq!(count(&mut app, first), 0);
    assert_eq!(count(&mut app, second), 2);
    assert!(app.world().get_entity(listener).is_ok());
}

#[test]
fn a_seeded_current_handle_cannot_bypass_explicit_path_validation() {
    let mut app = app();
    let scene = Handle::default();
    let root = app
        .world_mut()
        .spawn((
            MapSceneRequest {
                path: "plain.glb".into(),
            },
            MapSceneHandle {
                path: "plain.glb".into(),
                scene: scene.clone(),
            },
            bevy::world_serialization::WorldAssetRoot(scene),
        ))
        .id();
    app.update();
    assert!(matches!(
        app.world().get::<MapSceneStatus>(root),
        Some(MapSceneStatus::Failed(_))
    ));
}

#[test]
fn independent_root_records_keep_data_and_persistent_failures_separate() {
    let mut app = app();
    let good = app
        .world_mut()
        .spawn(MapSceneRequest {
            path: "ro://mini_map.glb".into(),
        })
        .id();
    let bad = app
        .world_mut()
        .spawn(MapSceneRequest {
            path: "ro://bad_version.glb".into(),
        })
        .id();
    settle(&mut app, good);
    settle(&mut app, bad);
    assert_eq!(
        app.world().get::<MapSceneStatus>(good),
        Some(&MapSceneStatus::Ready)
    );
    assert_eq!(
        app.world().get::<LifMapData>(good).unwrap().altitude.width,
        4
    );
    let failure = app.world().get::<MapSceneStatus>(bad).unwrap().clone();
    let MapSceneStatus::Failed(error) = &failure else {
        panic!("invalid root was accepted");
    };
    assert!(error.contains("ro://bad_version.glb"));
    assert!(app.world().get::<LifMapData>(bad).is_none());
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().get::<MapSceneStatus>(bad), Some(&failure));
}

#[test]
fn a_retained_pair_reuses_the_same_scene_but_a_mismatch_fails_explicitly() {
    let mut app = app();
    let first = app
        .world_mut()
        .spawn(MapSceneRequest {
            path: "ro://mini_map.glb".into(),
        })
        .id();
    settle(&mut app, first);
    let retained = app.world().get::<MapSceneHandle>(first).unwrap().clone();
    app.world_mut().entity_mut(first).despawn();
    let second = app
        .world_mut()
        .spawn((
            MapSceneRequest {
                path: retained.path.clone(),
            },
            retained.clone(),
        ))
        .id();
    settle(&mut app, second);
    assert_eq!(
        app.world().get::<MapSceneHandle>(second).unwrap().scene,
        retained.scene
    );
    assert_eq!(
        app.world().get::<MapSceneStatus>(second),
        Some(&MapSceneStatus::Ready)
    );
    let mismatch = app
        .world_mut()
        .spawn((
            MapSceneRequest {
                path: "ro://other.glb".into(),
            },
            retained,
        ))
        .id();
    settle(&mut app, mismatch);
    let Some(MapSceneStatus::Failed(error)) = app.world().get::<MapSceneStatus>(mismatch) else {
        panic!("mismatched retained pair was accepted");
    };
    assert!(error.contains("ro://other.glb") && error.contains("ro://mini_map.glb"));
}
