use bevy::prelude::*;
use game_engine::core::state::GameState;
use lifthrasir_runtime::{RuntimeCorePlugin, rendering::SpriteRenderingSystems};

#[derive(Resource, Default)]
struct PhaseTicks(usize);

#[test]
fn client_sprite_schedule_builds_with_gameplay_adapters() {
    let mut app = client_app();
    app.finish();
    app.cleanup();
    app.world_mut().schedule_scope(Update, |world, schedule| {
        schedule
            .initialize(world)
            .expect("client sprite/gameplay phase graph must be acyclic");
    });
}

fn client_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::state::app::StatesPlugin,
        ImagePlugin::default(),
        bevy::mesh::MeshPlugin,
        bevy::world_serialization::WorldSerializationPlugin,
        bevy::gltf::GltfPlugin::default(),
    ))
    .init_asset::<StandardMaterial>()
    .add_plugins(
        game_engine::CoreGamePlugins
            .build()
            .disable::<game_engine::AudioPlugin>()
            .disable::<game_engine::EffectsPlugin>()
            .disable::<game_engine::VfxPlugin>(),
    )
    .add_plugins(
        lifthrasir_runtime::ContentRuntimePlugins
            .build()
            .disable::<lifthrasir_runtime::ContentAudioPlugin>(),
    );
    app
}

#[test]
fn client_world_scene_and_character_phase_graph_is_acyclic() {
    let mut app = client_app();
    app.add_plugins(game_engine::domain::world::gltf_map::GltfMapPlugin);
    app.finish();
    app.cleanup();
    app.world_mut().schedule_scope(Update, |world, schedule| {
        schedule
            .initialize(world)
            .expect("selected-map activation and character readiness graph must be acyclic");
    });
}

#[test]
fn client_linked_runtime_core_does_not_install_client_state() {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .add_plugins(RuntimeCorePlugin);
    app.update();
    assert!(!app.world().contains_resource::<State<GameState>>());
}

#[test]
fn shared_rendering_phases_remain_active_during_client_loading() {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<PhaseTicks>()
        .add_plugins(bevy::state::app::StatesPlugin)
        .insert_state(GameState::Loading)
        .add_plugins(RuntimeCorePlugin)
        .add_systems(
            Update,
            (|mut ticks: ResMut<PhaseTicks>| ticks.0 += 1)
                .in_set(SpriteRenderingSystems::HierarchySpawn),
        );
    app.update();
    assert_eq!(app.world().resource::<PhaseTicks>().0, 1);
    assert_eq!(
        *app.world().resource::<State<GameState>>().get(),
        GameState::Loading
    );
}
