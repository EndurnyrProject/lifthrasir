use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetConfig {
    pub assets: AssetsSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetsSection {
    #[serde(default = "default_data_folder")]
    pub data_folder: String,
    #[serde(default)]
    pub archive: Vec<ArchiveConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveConfig {
    pub path: String,
    pub priority: u32,
}

fn default_data_folder() -> String {
    "./assets/data/".to_string()
}

impl Default for AssetConfig {
    fn default() -> Self {
        Self {
            assets: AssetsSection {
                data_folder: default_data_folder(),
                archive: vec![ArchiveConfig {
                    path: "lifthrasir.pak".to_string(),
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

    pub fn archives_by_priority(&self) -> Vec<&ArchiveConfig> {
        let mut archives: Vec<&ArchiveConfig> = self.assets.archive.iter().collect();
        archives.sort_by_key(|archive| archive.priority);
        archives
    }

    pub fn generate_default_config_content() -> String {
        r#"[assets]
data_folder = "./data/"

[[assets.archive]]
path = "lifthrasir.pak"
priority = 0
"#
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_default_config_parses_with_one_archive() {
        let content = AssetConfig::generate_default_config_content();
        let config: AssetConfig = toml::from_str(&content).unwrap();

        assert_eq!(config.assets.archive.len(), 1);
        assert_eq!(config.assets.archive[0].path, "lifthrasir.pak");
    }

    /// An old `[[grf]]` config must fail loudly rather than silently parse
    /// into zero archives: `deny_unknown_fields` turns the unrecognized `grf`
    /// key into a hard parse error, the intended migration signal.
    #[test]
    fn old_grf_schema_fails_to_parse() {
        let old_config = r#"[assets]
data_folder = "./data/"

[[assets.grf]]
path = "data.grf"
priority = 0
"#;
        let result: Result<AssetConfig, _> = toml::from_str(old_config);

        assert!(result.is_err());
    }
}
