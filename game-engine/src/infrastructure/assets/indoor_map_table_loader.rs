use bevy::{
    asset::{Asset, AssetLoader, LoadContext, io::Reader},
    prelude::*,
    reflect::TypePath,
};
use encoding_rs::EUC_KR;
use std::collections::HashSet;
use thiserror::Error;

/// Asset representing the indoor map table from `data\indoorrswtable.txt`.
///
/// Holds the set of map names (without extension, lowercased) that use the
/// restricted indoor camera (closer, fixed diagonal, no rotation).
#[derive(Asset, TypePath, Debug, Clone)]
pub struct IndoorMapTableAsset {
    pub maps: HashSet<String>,
}

/// Asset loader for the indoor map table.
/// Format: one indoor map per line as `<name>.rsw#`, with `//` comment lines.
#[derive(Default, TypePath)]
pub struct IndoorMapTableLoader;

#[derive(Debug, Error)]
pub enum IndoorMapTableLoaderError {
    #[error("Could not load indoor map table: {0}")]
    Io(#[from] std::io::Error),
}

impl AssetLoader for IndoorMapTableLoader {
    type Asset = IndoorMapTableAsset;
    type Settings = ();
    type Error = IndoorMapTableLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let (decoded, _, _) = EUC_KR.decode(&bytes);
        let maps = parse_indoor_table(&decoded);

        debug!("Indoor map table loaded: {} indoor maps", maps.len());

        Ok(IndoorMapTableAsset { maps })
    }

    fn extensions(&self) -> &[&str] {
        &["txt"]
    }
}

/// Parse the indoor map table into a set of normalized map names.
/// Each non-comment line is `<name>.rsw#`; names are lowercased without the
/// `.rsw` extension. Names may contain `@`, `-`, and digits (e.g. `1@gef_in`).
fn parse_indoor_table(content: &str) -> HashSet<String> {
    content.lines().filter_map(parse_indoor_entry).collect()
}

fn parse_indoor_entry(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return None;
    }

    let name = trimmed
        .trim_end_matches('#')
        .trim_end_matches(".rsw")
        .trim_end_matches(".RSW")
        .to_lowercase();

    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_and_skips_comments() {
        let content = "// header comment\n\nba_void.rsw#\nPRT_IN.rsw#\n";
        let maps = parse_indoor_table(content);
        assert!(maps.contains("ba_void"));
        assert!(maps.contains("prt_in"));
        assert_eq!(maps.len(), 2);
    }

    #[test]
    fn handles_special_chars_in_names() {
        let content = "1@gef_in.rsw#\nnew_1-2_evt.rsw#\nsword_1-1.rsw#\n";
        let maps = parse_indoor_table(content);
        assert!(maps.contains("1@gef_in"));
        assert!(maps.contains("new_1-2_evt"));
        assert!(maps.contains("sword_1-1"));
    }

    #[test]
    fn skips_blank_and_comment_only() {
        let maps = parse_indoor_table("//a\n   \n// b\n");
        assert!(maps.is_empty());
    }
}
