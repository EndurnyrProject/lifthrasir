use super::asset::ItemDataAsset;
use super::registry::ItemDb;
use bevy::asset::LoadState;
use bevy::prelude::*;

#[derive(Resource)]
struct ItemDataHandle(Handle<ItemDataAsset>);

pub struct ItemDbPlugin;

impl Plugin for ItemDbPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_loading_item_data)
            .add_systems(Update, process_loaded_item_data);
    }
}

fn start_loading_item_data(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load("data/ron/item_data.ron");
    commands.insert_resource(ItemDataHandle(handle));
    debug!("Loading item data RON");
}

fn process_loaded_item_data(
    mut commands: Commands,
    handle: Option<Res<ItemDataHandle>>,
    item_data_assets: Res<Assets<ItemDataAsset>>,
    asset_server: Res<AssetServer>,
) {
    let Some(handle) = handle else { return };

    if let LoadState::Failed(err) = asset_server.load_state(&handle.0) {
        error!(
            "Failed to load data/ron/item_data.ron: {:?}. Run `cargo run -p ro-to-lifthrasir-cli -- convert` to regenerate it.",
            err
        );
        commands.remove_resource::<ItemDataHandle>();
        return;
    }

    let Some(asset) = item_data_assets.get(&handle.0) else {
        return;
    };

    commands.insert_resource(ItemDb::from_item_data(asset.0.clone()));
    commands.remove_resource::<ItemDataHandle>();
    debug!("ItemDb created from RON");
}
