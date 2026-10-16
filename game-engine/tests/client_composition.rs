use bevy::prelude::*;
use lifthrasir_client::ClientPlugins;

#[test]
fn client_plugins_do_not_install_shared_or_network_groups() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::state::app::StatesPlugin,
    ))
    .init_asset::<Image>()
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .add_plugins(ClientPlugins);
    assert!(!app.is_plugin_added::<net_contract::NetContractPlugin>());
    assert!(!app.is_plugin_added::<lifthrasir_assets::ContentAssetsPlugin>());
    assert!(!app.is_plugin_added::<lifthrasir_assets::catalogs::ContentCatalogPlugin>());
    assert!(!app.is_plugin_added::<lifthrasir_runtime::RuntimeCorePlugin>());
    assert!(!app.is_plugin_added::<lifthrasir_runtime::ActorRenderPlugin>());
    assert!(!app.is_plugin_added::<lifthrasir_runtime::EffectsPlaybackPlugin>());
    assert!(!app.is_plugin_added::<lifthrasir_runtime::ContentAudioPlugin>());
    assert!(!app.is_plugin_added::<lifthrasir_runtime::WorldRenderPlugin>());
    assert!(
        !app.world()
            .contains_resource::<Messages<net_contract::events::LoginAccepted>>()
    );
}
