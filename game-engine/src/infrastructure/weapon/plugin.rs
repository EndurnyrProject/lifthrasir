use super::asset::WeaponDataAsset;
use super::registry::WeaponDb;
use bevy::asset::LoadState;
use bevy::prelude::*;

#[derive(Resource)]
struct WeaponDataHandle(Handle<WeaponDataAsset>);

pub struct WeaponDbPlugin;

impl Plugin for WeaponDbPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_loading_weapon_data)
            .add_systems(Update, process_loaded_weapon_data);
    }
}

fn start_loading_weapon_data(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load("data/ron/weapon_data.ron");
    commands.insert_resource(WeaponDataHandle(handle));
    debug!("Loading weapon data RON");
}

fn process_loaded_weapon_data(
    mut commands: Commands,
    handle: Option<Res<WeaponDataHandle>>,
    weapon_data_assets: Res<Assets<WeaponDataAsset>>,
    asset_server: Res<AssetServer>,
) {
    let Some(handle) = handle else { return };

    if let LoadState::Failed(err) = asset_server.load_state(&handle.0) {
        error!(
            "Failed to load data/ron/weapon_data.ron: {:?}. Run `cargo run -p ro-to-lifthrasir-cli -- convert` to regenerate it.",
            err
        );
        commands.remove_resource::<WeaponDataHandle>();
        return;
    }

    let Some(asset) = weapon_data_assets.get(&handle.0) else {
        return;
    };

    commands.insert_resource(WeaponDb::from_weapon_data(asset.0.clone()));
    commands.remove_resource::<WeaponDataHandle>();
    debug!("WeaponDb created from RON");
}
