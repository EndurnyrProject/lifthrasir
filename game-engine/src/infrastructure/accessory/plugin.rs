use super::asset::AccessoryDataAsset;
use super::registry::AccessoryDb;
use bevy::asset::LoadState;
use bevy::prelude::*;

#[derive(Resource)]
struct AccessoryDataHandle(Handle<AccessoryDataAsset>);

pub struct AccessoryDbPlugin;

impl Plugin for AccessoryDbPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_loading_accessory_data)
            .add_systems(Update, process_loaded_accessory_data);
    }
}

fn start_loading_accessory_data(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load("data/ron/accessory_data.ron");
    commands.insert_resource(AccessoryDataHandle(handle));
    debug!("Loading accessory data RON");
}

fn process_loaded_accessory_data(
    mut commands: Commands,
    handle: Option<Res<AccessoryDataHandle>>,
    accessory_data_assets: Res<Assets<AccessoryDataAsset>>,
    asset_server: Res<AssetServer>,
) {
    let Some(handle) = handle else { return };

    if let LoadState::Failed(err) = asset_server.load_state(&handle.0) {
        error!(
            "Failed to load data/ron/accessory_data.ron: {:?}. Run `cargo run -p ro-to-lifthrasir-cli -- convert` to regenerate it.",
            err
        );
        commands.remove_resource::<AccessoryDataHandle>();
        return;
    }

    let Some(asset) = accessory_data_assets.get(&handle.0) else {
        return;
    };

    commands.insert_resource(AccessoryDb::from_accessory_data(asset.0.clone()));
    commands.remove_resource::<AccessoryDataHandle>();
    debug!("AccessoryDb created from RON");
}
