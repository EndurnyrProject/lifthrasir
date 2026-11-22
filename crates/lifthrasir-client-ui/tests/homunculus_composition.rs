//! Headless composition: public actors alone don't grant owner commands.

use bevy::{asset::io::AssetSourceBuilder, prelude::*, scene::ScenePlugin};
use leafwing_input_manager::prelude::ActionState;
use lifthrasir_assets::loaders::{RoActAsset, RoSpriteAsset};
use lifthrasir_client::{
    core::state::GameState,
    domain::{
        entities::{
            markers::{Homunculus, LocalPlayer},
            registry::EntityRegistry,
        },
        homunculus::{HomunculusIntent, HomunculusPlugin, HomunculusState},
        input::{PlayerAction, UiFocus},
    },
};
use lifthrasir_client_ui::widgets::homunculus_window::{HomunculusWindowPlugin, spawn_window};
use net_contract::{
    commands::HomunculusCommand,
    dto::{HomunculusAction, HomunculusLifecycle, HomunculusSnapshot},
    events::{HomunculusIngress, HomunculusIngressPayload, HomunculusReady},
    state::ZoneSessionGeneration,
};

#[test]
fn homunculus_composition_keeps_public_and_private_paths_separate() {
    let mut app = App::new();
    app.register_asset_source("ro", AssetSourceBuilder::platform_default("", None));
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ScenePlugin,
        bevy::state::app::StatesPlugin,
        net_contract::NetContractPlugin,
        HomunculusPlugin,
        HomunculusWindowPlugin,
    ));
    app.init_state::<GameState>()
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_asset::<RoSpriteAsset>()
        .init_asset::<RoActAsset>()
        .init_resource::<EntityRegistry>()
        .init_resource::<UiFocus>();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::InGame);
    let hud = app.world_mut().spawn_empty().id();
    spawn_window(&mut app.world_mut().commands(), hud);
    let mut keys = ActionState::<PlayerAction>::default();
    keys.press(&PlayerAction::Homunculus);
    let player = app.world_mut().spawn((LocalPlayer, keys)).id();
    let actor = app.world_mut().spawn(Homunculus).id();
    app.world_mut()
        .resource_mut::<EntityRegistry>()
        .register_entity(71, actor);
    app.update();
    app.world_mut()
        .get_mut::<ActionState<PlayerAction>>(player)
        .unwrap()
        .release(&PlayerAction::Homunculus);

    let generation = ZoneSessionGeneration::default();
    app.world_mut()
        .write_message(HomunculusReady { generation });
    app.world_mut().write_message(HomunculusIntent::Act {
        durable_id: 99,
        action: HomunculusAction::Delete { confirmed: true },
    });
    app.update();
    assert!(
        app.world()
            .resource::<HomunculusState>()
            .controlled_gid()
            .is_none()
    );
    assert!(
        app.world()
            .resource::<Messages<HomunculusCommand>>()
            .iter_current_update_messages()
            .all(|c| c.action == HomunculusAction::Inspect)
    );

    app.world_mut().write_message(HomunculusIngress {
        generation,
        sequence: 2,
        payload: HomunculusIngressPayload::State(HomunculusSnapshot {
            durable_id: 99,
            world_gid: Some(71),
            name: "Test companion".into(),
            lifecycle: HomunculusLifecycle::Active,
            hp: 50,
            max_hp: 60,
            ..default()
        }),
    });
    app.update();
    assert_eq!(
        app.world().resource::<HomunculusState>().controlled_gid(),
        Some(71)
    );
    let mut labels = app.world_mut().query::<&Text>();
    assert!(
        labels
            .iter(app.world())
            .any(|text| text.0.contains("Test companion") && text.0.contains("HP 50 / 60"))
    );

    app.world_mut().write_message(HomunculusIngress {
        generation,
        sequence: 3,
        payload: HomunculusIngressPayload::MapBarrier,
    });
    app.update();
    assert!(app.world().get::<Homunculus>(actor).is_some());
    assert!(
        app.world()
            .resource::<HomunculusState>()
            .controlled_gid()
            .is_none()
    );
}
