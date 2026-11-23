use super::asset::QuestAsset;
use super::registry::QuestDb;
use bevy::asset::LoadState;
use bevy::prelude::*;

#[derive(Resource)]
struct QuestDataHandle(Handle<QuestAsset>);

pub struct QuestDbPlugin;

impl Plugin for QuestDbPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_loading_quest_data)
            .add_systems(Update, process_loaded_quest_data);
    }
}

fn start_loading_quest_data(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load("ro://ron/quest_data.ron");
    commands.insert_resource(QuestDataHandle(handle));
    debug!("Loading quest data RON");
}

fn process_loaded_quest_data(
    mut commands: Commands,
    handle: Option<Res<QuestDataHandle>>,
    quest_assets: Res<Assets<QuestAsset>>,
    asset_server: Res<AssetServer>,
) {
    let Some(handle) = handle else { return };

    if let LoadState::Failed(err) = asset_server.load_state(&handle.0) {
        error!(
            "Failed to load ro://ron/quest_data.ron: {err:?}. Run `cargo run -p ro-to-lifthrasir-cli -- convert --only quests` to regenerate it."
        );
        commands.remove_resource::<QuestDataHandle>();
        return;
    }

    let Some(asset) = quest_assets.get(&handle.0) else {
        return;
    };

    commands.insert_resource(QuestDb::from_data(asset.0.clone()));
    commands.remove_resource::<QuestDataHandle>();
    debug!("QuestDb created from RON");
}
