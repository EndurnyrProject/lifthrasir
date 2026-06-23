use super::asset::JobDataAsset;
use super::registry::JobSpriteRegistry;
use bevy::asset::LoadState;
use bevy::prelude::*;

#[derive(Resource)]
struct JobDataHandle(Handle<JobDataAsset>);

pub struct JobSystemPlugin;

impl Plugin for JobSystemPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_loading_job_data)
            .add_systems(Update, process_loaded_job_data);
    }
}

fn start_loading_job_data(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load("data/ron/job_data.ron");
    commands.insert_resource(JobDataHandle(handle));
    debug!("Loading job data RON");
}

fn process_loaded_job_data(
    mut commands: Commands,
    handle: Option<Res<JobDataHandle>>,
    job_data_assets: Res<Assets<JobDataAsset>>,
    asset_server: Res<AssetServer>,
) {
    let Some(handle) = handle else { return };

    if let LoadState::Failed(err) = asset_server.load_state(&handle.0) {
        error!(
            "Failed to load data/ron/job_data.ron: {:?}. Run `cargo run -p ro-to-lifthrasir-cli -- convert` to regenerate it.",
            err
        );
        commands.remove_resource::<JobDataHandle>();
        return;
    }

    let Some(asset) = job_data_assets.get(&handle.0) else {
        return;
    };

    commands.insert_resource(JobSpriteRegistry::from_job_data(asset.0.clone()));
    commands.remove_resource::<JobDataHandle>();
    debug!("Job sprite registry created from RON");
}
