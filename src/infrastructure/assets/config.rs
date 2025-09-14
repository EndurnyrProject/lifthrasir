use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Asset, TypePath, Debug, Clone, Serialize, Deserialize)]
pub struct AssetConfig {
    pub assets: AssetsSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetsSection {
    #[serde(default = "default_data_folder")]
    pub data_folder: String,
    #[serde(default)]
    pub grf: Vec<GrfConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrfConfig {
    pub path: String,
    pub priority: u32,
}

fn default_data_folder() -> String {
    "./data/".to_string()
}

impl Default for AssetConfig {
    fn default() -> Self {
        Self {
            assets: AssetsSection {
                data_folder: default_data_folder(),
                grf: vec![GrfConfig {
                    path: "data.grf".to_string(),
                    priority: 0,
                }],
            },
        }
    }
}

impl AssetConfig {
    pub fn data_folder_path(&self) -> PathBuf {
        PathBuf::from(&self.assets.data_folder)
    }

    pub fn grf_files_by_priority(&self) -> Vec<&GrfConfig> {
        let mut grf_files: Vec<&GrfConfig> = self.assets.grf.iter().collect();
        grf_files.sort_by_key(|grf| grf.priority);
        grf_files
    }

    pub fn generate_default_config_content() -> String {
        r#"[assets]
data_folder = "./data/"

[[grf]]
path = "data.grf"
priority = 0

# Example additional GRF files:
# [[grf]]
# path = "sdata.grf"  
# priority = 1
#
# [[grf]]
# path = "rdata.grf"
# priority = 2
"#
        .to_string()
    }

    pub fn save_default_config(
        config_path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::fs;
        let content = Self::generate_default_config_content();
        fs::write(config_path, content)?;
        Ok(())
    }
}

#[derive(Resource, Debug)]
pub struct AssetConfigHandle {
    pub handle: Handle<AssetConfig>,
    pub loaded: bool,
}

impl AssetConfigHandle {
    pub fn new(handle: Handle<AssetConfig>) -> Self {
        Self {
            handle,
            loaded: false,
        }
    }
}

pub fn check_config_loaded(
    mut config_handle: ResMut<AssetConfigHandle>,
    configs: Res<Assets<AssetConfig>>,
) {
    if !config_handle.loaded {
        if configs.get(&config_handle.handle).is_some() {
            config_handle.loaded = true;
            info!("Asset configuration loaded successfully");
        }
    }
}
