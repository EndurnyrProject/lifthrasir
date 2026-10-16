use bevy::prelude::*;
use lifthrasir_runtime::actors::{ActorEmblem, ActorVisualStatus, Gr2Actor};

#[test]
fn failed_glb_load_reports_the_path_without_a_client_failure_policy() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .add_plugins((
            lifthrasir_assets::ContentAssetsPlugin,
            lifthrasir_runtime::RuntimeCorePlugin,
            lifthrasir_runtime::ActorRenderPlugin,
        ));
    let path = "missing-model-source://required.glb";
    let actor = app.world_mut().spawn(Gr2Actor { model: path.into() }).id();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        app.update();
        if let Some(ActorVisualStatus::Failed(error)) = app.world().get::<ActorVisualStatus>(actor)
        {
            assert!(error.contains(path));
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "model load failure was not reported"
        );
        std::thread::yield_now();
    }
    assert_eq!(
        app.world_mut().query::<&Window>().iter(app.world()).count(),
        0
    );
}

#[test]
fn model_inputs_require_only_neutral_presentation_state() {
    let mut world = World::new();
    let actor = world
        .spawn((
            Gr2Actor {
                model: "ro://models/3dmob/test.glb".into(),
            },
            ActorEmblem(None),
        ))
        .id();
    assert_eq!(
        world.get::<ActorVisualStatus>(actor),
        Some(&ActorVisualStatus::Loading)
    );
    assert!(
        world
            .get::<lifthrasir_runtime::actors::ActorAnimationInput>(actor)
            .is_some()
    );
    assert!(world.get::<Transform>(actor).is_some());
    assert!(world.get::<Visibility>(actor).is_some());
}
