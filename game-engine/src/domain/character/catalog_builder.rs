use crate::domain::assets::{normalize_path, parse_gender, HAIR_PALETTE, HAIR_SPRITE};
use crate::domain::character::catalog::{HeadStyleCatalog, HeadStyleEntry};
use crate::domain::entities::character::components::Gender;
use crate::infrastructure::assets::HierarchicalAssetManager;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_system;
use std::collections::HashMap;

type StyleKey = (Gender, u16);

pub struct HeadStyleCatalogBuilder;

impl HeadStyleCatalogBuilder {
    /// Parses a sprite file path and returns a StyleKey and HeadStyleEntry if valid.
    /// Returns None if the path doesn't match the expected pattern, has parsing errors,
    /// or is missing the corresponding .act file.
    fn parse_sprite_entry(
        path: &str,
        all_sprite_paths: &[String],
    ) -> Option<(StyleKey, HeadStyleEntry)> {
        let normalized_path = normalize_path(path);

        let caps = HAIR_SPRITE.captures(&normalized_path)?;
        let gender_str = caps.get(1)?;
        let id_str = caps.get(2)?;

        let gender = parse_gender(gender_str.as_str())?;
        let id = id_str.as_str().parse::<u16>().ok()?;

        let act_path = path.replace(".spr", ".act");
        if !all_sprite_paths.iter().any(|p| p == &act_path) {
            debug!(
                "Found sprite {} but missing corresponding .act file: {}",
                path, act_path
            );
            return None;
        }

        let sprite_path = format!("ro://{}", path.replace('\\', "/"));
        let act_path_with_prefix = format!("ro://{}", act_path.replace('\\', "/"));

        let entry = HeadStyleEntry {
            id,
            gender,
            sprite_path,
            act_path: act_path_with_prefix,
            available_colors: vec![0], // Default color
        };

        Some(((gender, id), entry))
    }

    /// Parses a palette file path and returns a StyleKey and color ID if valid.
    /// Returns None if the path doesn't match the expected pattern or has parsing errors.
    fn parse_palette_entry(path: &str) -> Option<(StyleKey, u16)> {
        let normalized_path = normalize_path(path);

        let caps = HAIR_PALETTE.captures(&normalized_path)?;
        let id_str = caps.get(1)?;
        let gender_str = caps.get(2)?;
        let color_str = caps.get(3)?;

        let id = id_str.as_str().parse::<u16>().ok()?;
        let gender = parse_gender(gender_str.as_str())?;
        let color = color_str.as_str().parse::<u16>().ok()?;

        Some(((gender, id), color))
    }

    pub fn build_from_asset_manager(manager: &HierarchicalAssetManager) -> HeadStyleCatalog {
        let mut catalog = HeadStyleCatalog::new();

        info!("Starting head style catalog discovery from asset manager...");

        let all_files = manager.list_files();
        let sprite_prefix = "data/sprite/인간족/머리통";
        let palette_prefix = "data/palette/머리";

        let all_sprite_paths: Vec<String> = all_files
            .iter()
            .filter(|path| path.starts_with(sprite_prefix))
            .map(|s| s.to_string())
            .collect();

        let all_palette_paths: Vec<String> = all_files
            .iter()
            .filter(|path| path.starts_with(palette_prefix))
            .map(|s| s.to_string())
            .collect();

        debug!(
            "Scanning {} sprite files and {} palette files",
            all_sprite_paths.len(),
            all_palette_paths.len()
        );

        // Process sprites using iterator combinator
        let mut temp_entries: HashMap<StyleKey, HeadStyleEntry> = all_sprite_paths
            .iter()
            .filter_map(|path| Self::parse_sprite_entry(path, &all_sprite_paths))
            .collect();

        debug!("Found {} valid hair sprites", temp_entries.len());

        // Process palettes
        let mut palette_map: HashMap<StyleKey, Vec<u16>> = HashMap::new();
        for path in &all_palette_paths {
            if let Some((key, color)) = Self::parse_palette_entry(path) {
                palette_map.entry(key).or_default().push(color);
            }
        }

        debug!(
            "Found {} palette files",
            palette_map.values().map(|v| v.len()).sum::<usize>()
        );

        // Merge palettes into entries
        for (key, colors) in palette_map {
            if let Some(entry) = temp_entries.get_mut(&key) {
                entry.available_colors.extend(colors);
                entry.available_colors.sort();
                entry.available_colors.dedup();
            } else {
                debug!(
                    "Found palettes for style {} ({:?}) but no corresponding sprite",
                    key.1, key.0
                );
            }
        }

        for entry in temp_entries.into_values() {
            catalog.add(entry);
        }

        info!(
            "Head style catalog built: {} male styles, {} female styles, {} total",
            catalog.male_count(),
            catalog.female_count(),
            catalog.total_count()
        );

        catalog
    }
}

/// System to build catalog on startup
#[auto_add_system(
    plugin = crate::domain::character::catalog_builder::AssetCatalogPlugin,
    schedule = Update
)]
fn build_catalog_on_startup(
    mut commands: Commands,
    asset_manager: Option<Res<HierarchicalAssetManager>>,
    mut initialized: Local<bool>,
) {
    if *initialized {
        return;
    }

    let Some(asset_manager) = asset_manager else {
        return;
    };

    let head_catalog = HeadStyleCatalogBuilder::build_from_asset_manager(&asset_manager);

    commands.insert_resource(head_catalog);

    *initialized = true;
}

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct AssetCatalogPlugin;
