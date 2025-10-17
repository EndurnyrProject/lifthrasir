use bevy::{
    asset::{io::Reader, Asset, AssetLoader, LoadContext},
    log::info,
    prelude::*,
    reflect::TypePath,
};
use encoding_rs::EUC_KR;
use std::collections::HashMap;
use thiserror::Error;

/// Asset representing the BGM name table from mp3nametable.txt
/// Maps map names (without .rsw extension) to BGM file paths
#[derive(Asset, TypePath, Debug, Clone)]
pub struct BgmNameTableAsset {
    pub table: HashMap<String, String>,
}

/// Asset loader for BGM name table files
/// Parses the mp3nametable.txt format: `<map>.rsw#<bgm_path>#`
#[derive(Default)]
pub struct BgmNameTableLoader;

/// Errors that can occur when loading BGM name table
#[derive(Debug, Error)]
pub enum BgmNameTableLoaderError {
    #[error("Could not load BGM name table: {0}")]
    Io(#[from] std::io::Error),
}

impl AssetLoader for BgmNameTableLoader {
    type Asset = BgmNameTableAsset;
    type Settings = ();
    type Error = BgmNameTableLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let (decoded, _, _) = EUC_KR.decode(&bytes);
        let content = decoded.into_owned();

        let mut table = HashMap::new();
        let mut parsed_count = 0;
        let mut skipped_count = 0;

        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments (lines starting with //)
            if trimmed.starts_with("//") {
                skipped_count += 1;
                continue;
            }

            // Skip empty lines
            if trimmed.is_empty() {
                skipped_count += 1;
                continue;
            }

            // Parse format: <map>.rsw#<bgm_path>#
            if let Some(entry) = parse_bgm_entry(trimmed) {
                table.insert(entry.map_name, entry.bgm_path);
                parsed_count += 1;
            } else {
                info!(
                    "BGM name table: Failed to parse line {}: '{}'",
                    line_num + 1,
                    trimmed
                );
                skipped_count += 1;
            }
        }

        info!(
            "BGM name table loaded: {} entries parsed, {} lines skipped",
            parsed_count, skipped_count
        );

        Ok(BgmNameTableAsset { table })
    }

    fn extensions(&self) -> &[&str] {
        &["txt"]
    }
}

/// Parsed BGM table entry
struct BgmEntry {
    map_name: String,
    bgm_path: String,
}

/// Parse a single BGM name table entry
/// Format: `<map>.rsw#<bgm_path>#`
/// Example: `prontera.rsw#bgm\\08.mp3#`
fn parse_bgm_entry(line: &str) -> Option<BgmEntry> {
    // Split by '#' delimiter
    let parts: Vec<&str> = line.split('#').collect();

    // Need at least 2 parts: map name and bgm path (third '#' is just delimiter)
    if parts.len() < 2 {
        return None;
    }

    let map_part = parts[0].trim();
    let bgm_part = parts[1].trim();

    // Validate both parts are non-empty
    if map_part.is_empty() || bgm_part.is_empty() {
        return None;
    }

    // Normalize map name: remove .rsw suffix and convert to lowercase
    let map_name = map_part
        .trim_end_matches(".rsw")
        .trim_end_matches(".RSW")
        .to_lowercase();

    // Normalize BGM path: replace backslashes with forward slashes and collapse consecutive slashes
    let bgm_path = bgm_part.replace('\\', "/").replace("//", "/");

    Some(BgmEntry { map_name, bgm_path })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bgm_entry_valid() {
        let entry = parse_bgm_entry("prontera.rsw#bgm\\08.mp3#").unwrap();
        assert_eq!(entry.map_name, "prontera");
        assert_eq!(entry.bgm_path, "bgm/08.mp3");
    }

    #[test]
    fn test_parse_bgm_entry_uppercase() {
        let entry = parse_bgm_entry("PRT_CHURCH.RSW#BGM\\10.mp3#").unwrap();
        assert_eq!(entry.map_name, "prt_church");
        assert_eq!(entry.bgm_path, "BGM/10.mp3");
    }

    #[test]
    fn test_parse_bgm_entry_forward_slash() {
        let entry = parse_bgm_entry("izlude.rsw#bgm/26.mp3#").unwrap();
        assert_eq!(entry.map_name, "izlude");
        assert_eq!(entry.bgm_path, "bgm/26.mp3");
    }

    #[test]
    fn test_parse_bgm_entry_invalid_format() {
        assert!(parse_bgm_entry("invalid_format").is_none());
        assert!(parse_bgm_entry("only_one_part#").is_none());
        assert!(parse_bgm_entry("#empty_map").is_none());
        assert!(parse_bgm_entry("map.rsw##").is_none());
    }

    #[test]
    fn test_parse_bgm_entry_with_whitespace() {
        let entry = parse_bgm_entry("  prontera.rsw  #  bgm\\08.mp3  #  ").unwrap();
        assert_eq!(entry.map_name, "prontera");
        assert_eq!(entry.bgm_path, "bgm/08.mp3");
    }
}
