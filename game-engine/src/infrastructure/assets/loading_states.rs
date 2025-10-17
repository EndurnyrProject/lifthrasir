use super::{AssetConfig, HierarchicalAssetManager};
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum AssetLoadingState {
    #[default]
    LoadingConfig,
    SettingUpSources,
    LoadingAssets,
    Ready,
    Error,
}

#[derive(AssetCollection, Resource)]
pub struct ConfigAssets {
    #[asset(path = "loader.data.toml")]
    pub config: Handle<AssetConfig>,
}

pub struct AssetLoadingPlugin;

impl Plugin for AssetLoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AssetLoadingState>()
            .add_loading_state(
                LoadingState::new(AssetLoadingState::LoadingConfig)
                    .continue_to_state(AssetLoadingState::SettingUpSources)
                    .load_collection::<ConfigAssets>(),
            )
            .add_systems(
                OnEnter(AssetLoadingState::SettingUpSources),
                setup_hierarchical_manager,
            )
            .add_systems(
                Update,
                (
                    check_manager_setup.run_if(in_state(AssetLoadingState::SettingUpSources)),
                    monitor_loading_progress.run_if(in_state(AssetLoadingState::LoadingAssets)),
                ),
            );
    }
}

fn setup_hierarchical_manager(
    mut commands: Commands,
    config_assets: Res<ConfigAssets>,
    configs: Res<Assets<AssetConfig>>,
    mut next_state: ResMut<NextState<AssetLoadingState>>,
) {
    if let Some(config) = configs.get(&config_assets.config) {
        info!("Setting up hierarchical asset manager from config");

        match HierarchicalAssetManager::from_config(config) {
            Ok(manager) => {
                info!("Hierarchical asset manager setup successful");
                commands.insert_resource(manager);
                next_state.set(AssetLoadingState::LoadingAssets);
            }
            Err(e) => {
                error!("Failed to setup hierarchical asset manager: {}", e);
                next_state.set(AssetLoadingState::Error);
            }
        }
    } else {
        warn!("Config asset not yet loaded, will retry next frame");
    }
}

fn check_manager_setup(
    manager: Option<Res<HierarchicalAssetManager>>,
    mut next_state: ResMut<NextState<AssetLoadingState>>,
) {
    if manager.is_some() {
        info!("Hierarchical asset manager ready, proceeding to asset loading");
        next_state.set(AssetLoadingState::LoadingAssets);
    }
}

fn monitor_loading_progress(
    manager: Option<Res<HierarchicalAssetManager>>,
    mut next_state: ResMut<NextState<AssetLoadingState>>,
) {
    if let Some(_manager) = manager {
        info!("Asset loading complete, system ready");
        next_state.set(AssetLoadingState::Ready);
    }
}

// System to create default config if it doesn't exist
pub fn ensure_default_config() -> std::io::Result<()> {
    use std::path::Path;

    let config_path = Path::new("assets/loader.data.toml");

    if !config_path.exists() {
        info!("loader.data.toml not found, creating default configuration");

        // Ensure assets directory exists
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Create default config
        AssetConfig::save_default_config(config_path)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        info!("Created default data.toml at: {}", config_path.display());
    }

    Ok(())
}

// Resource to track loading progress
#[derive(Resource, Default)]
pub struct LoadingProgress {
    pub total_steps: u32,
    pub completed_steps: u32,
    pub current_step: String,
    pub errors: Vec<String>,
}

impl LoadingProgress {
    pub fn new(total_steps: u32) -> Self {
        Self {
            total_steps,
            completed_steps: 0,
            current_step: "Initializing...".to_string(),
            errors: Vec::new(),
        }
    }

    pub fn advance(&mut self, step_description: &str) {
        self.completed_steps += 1;
        self.current_step = step_description.to_string();
        info!(
            "Loading progress: {}/{} - {}",
            self.completed_steps, self.total_steps, step_description
        );
    }

    pub fn add_error(&mut self, error: String) {
        error!("Loading error: {}", error);
        self.errors.push(error);
    }

    pub fn is_complete(&self) -> bool {
        self.completed_steps >= self.total_steps
    }

    pub fn progress_percentage(&self) -> f32 {
        if self.total_steps == 0 {
            100.0
        } else {
            (self.completed_steps as f32 / self.total_steps as f32) * 100.0
        }
    }
}
