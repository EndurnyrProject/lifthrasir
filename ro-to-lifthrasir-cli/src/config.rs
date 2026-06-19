use anyhow::Context;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct LoaderConfig {
    pub assets: AssetsSection,
}

#[derive(Debug, Deserialize)]
pub struct AssetsSection {
    pub grf: Vec<GrfEntry>,
}

#[derive(Debug, Deserialize)]
pub struct GrfEntry {
    pub path: String,
    pub priority: u32,
}

impl LoaderConfig {
    pub fn from_path(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading loader config: {}", path.display()))?;
        toml::from_str(&content)
            .with_context(|| format!("parsing loader config: {}", path.display()))
    }

    pub fn grfs_by_priority(&self) -> Vec<&GrfEntry> {
        let mut grfs: Vec<&GrfEntry> = self.assets.grf.iter().collect();
        grfs.sort_by_key(|g| g.priority);
        grfs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grfs_sorted_ascending_by_priority() {
        let config: LoaderConfig = toml::from_str(
            r#"
[assets]
data_folder = "assets/data"

[[assets.grf]]
path = "en.grf"
priority = 1

[[assets.grf]]
path = "data.grf"
priority = 0
"#,
        )
        .unwrap();

        let grfs = config.grfs_by_priority();
        assert_eq!(grfs.len(), 2);
        assert_eq!(grfs[0].path, "data.grf");
        assert_eq!(grfs[1].path, "en.grf");
    }
}
