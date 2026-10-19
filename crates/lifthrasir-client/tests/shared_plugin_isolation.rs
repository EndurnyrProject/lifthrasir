use bevy::{animation::AnimationPlugin, input::InputPlugin, prelude::*};
use lifthrasir_client::{
    ClientPlugins,
    core::state::GameState,
    domain::{
        effects::PendingBodyStates, entities::registry::EntityRegistry, input::UiFocus,
        settings::GraphicsSettings,
    },
};

#[test]
fn linked_client_cannot_auto_register_into_shared_cpu_plugins() {
    let _linked = std::hint::black_box(ClientPlugins.build());
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        InputPlugin,
        ImagePlugin::default(),
        bevy::mesh::MeshPlugin,
        AnimationPlugin,
        bevy::world_serialization::WorldSerializationPlugin,
        bevy::gltf::GltfPlugin::default(),
    ))
    .init_asset::<Font>()
    .init_asset::<bevy::shader::Shader>()
    .add_plugins((
        lifthrasir_assets::ContentAssetsPlugin,
        lifthrasir_assets::catalogs::ContentCatalogPlugin,
    ))
    .add_plugins(
        lifthrasir_runtime::ContentRuntimePlugins
            .build()
            .disable::<lifthrasir_runtime::ContentAudioPlugin>(),
    )
    .add_plugins(lifthrasir_ui::UiPrimitivesPlugin);
    app.finish();
    app.cleanup();
    app.update();
    app.update();
    let mut named_runtime_handlers = 0;
    let labels: Vec<_> = app
        .world()
        .resource::<bevy::ecs::schedule::Schedules>()
        .iter()
        .map(|(_, schedule)| schedule.label())
        .collect();
    for label in labels {
        app.world_mut().schedule_scope(label, |world, schedule| {
            schedule.initialize(world).unwrap();
            for (_, system) in schedule.systems().unwrap() {
                named_runtime_handlers +=
                    usize::from(system.name().contains("lifthrasir_runtime::"));
                assert!(
                    !system.name().contains("lifthrasir_client::"),
                    "client handler leaked into shared host: {}",
                    system.name()
                );
            }
        });
    }
    for observer in app.world_mut().query::<&Observer>().iter(app.world()) {
        assert!(
            !observer.system_name().contains("lifthrasir_client::"),
            "client observer leaked into shared host: {}",
            observer.system_name()
        );
    }
    for (info, _) in app.world().iter_resources() {
        assert!(
            !info.name().contains("lifthrasir_client::") && !info.name().contains("net_contract::"),
            "client/network resource leaked into shared host: {}",
            info.name()
        );
    }
    assert!(
        named_runtime_handlers > 0,
        "test requires Bevy debug system names, not stripped placeholders"
    );
    assert!(!app.world().contains_resource::<State<GameState>>());
    assert!(!app.world().contains_resource::<EntityRegistry>());
    assert!(!app.world().contains_resource::<UiFocus>());
    assert!(!app.world().contains_resource::<GraphicsSettings>());
    assert!(!app.world().contains_resource::<PendingBodyStates>());
    assert!(
        !app.world()
            .contains_resource::<net_contract::state::UserSession>()
    );
    assert!(
        !app.world()
            .contains_resource::<Messages<net_contract::events::LoginAccepted>>()
    );
    assert!(!app.is_plugin_added::<net_contract::NetContractPlugin>());
}
