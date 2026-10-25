use super::asset::RandomOptionDataAsset;
use super::registry::RandomOptionDb;
use bevy::asset::LoadState;
use bevy::prelude::*;

#[derive(Resource)]
struct RandomOptionDataHandle(Handle<RandomOptionDataAsset>);

pub struct RandomOptionDbPlugin;

impl Plugin for RandomOptionDbPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_loading_random_option_data)
            .add_systems(Update, process_loaded_random_option_data);
    }
}

fn start_loading_random_option_data(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load("ro://ron/random_option_data.ron");
    commands.insert_resource(RandomOptionDataHandle(handle));
    debug!("Loading random option data RON");
}

fn process_loaded_random_option_data(
    mut commands: Commands,
    handle: Option<Res<RandomOptionDataHandle>>,
    option_data_assets: Res<Assets<RandomOptionDataAsset>>,
    asset_server: Res<AssetServer>,
) {
    let Some(handle) = handle else { return };

    if let LoadState::Failed(err) = asset_server.load_state(&handle.0) {
        error!(
            "Failed to load ro://ron/random_option_data.ron: {:?}. Run `cargo run -p ro-to-lifthrasir-cli -- convert` to regenerate it.",
            err
        );
        commands.remove_resource::<RandomOptionDataHandle>();
        return;
    }

    let Some(asset) = option_data_assets.get(&handle.0) else {
        return;
    };

    commands.insert_resource(RandomOptionDb::from_random_option_data(asset.0.clone()));
    commands.remove_resource::<RandomOptionDataHandle>();
    debug!("RandomOptionDb created from RON");
}
