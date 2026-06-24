use super::asset::SkillDataAsset;
use super::registry::SkillCatalog;
use bevy::asset::LoadState;
use bevy::prelude::*;

#[derive(Resource)]
struct SkillDataHandle(Handle<SkillDataAsset>);

pub struct SkillSystemPlugin;

impl Plugin for SkillSystemPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_loading_skill_data)
            .add_systems(Update, process_loaded_skill_data);
    }
}

fn start_loading_skill_data(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load("data/ron/skill_data.ron");
    commands.insert_resource(SkillDataHandle(handle));
    debug!("Loading skill data RON");
}

fn process_loaded_skill_data(
    mut commands: Commands,
    handle: Option<Res<SkillDataHandle>>,
    skill_data_assets: Res<Assets<SkillDataAsset>>,
    asset_server: Res<AssetServer>,
) {
    let Some(handle) = handle else { return };

    if let LoadState::Failed(err) = asset_server.load_state(&handle.0) {
        error!(
            "Failed to load data/ron/skill_data.ron: {:?}. Run `cargo run -p ro-to-lifthrasir-cli -- convert` to regenerate it.",
            err
        );
        commands.remove_resource::<SkillDataHandle>();
        return;
    }

    let Some(asset) = skill_data_assets.get(&handle.0) else {
        return;
    };

    commands.insert_resource(SkillCatalog::from_skill_data(asset.0.clone()));
    commands.remove_resource::<SkillDataHandle>();
    debug!("Skill catalog created from RON");
}
