use crate::domain::assets::{normalize_path, parse_gender, HAIR_PALETTE, HAIR_SPRITE};
use crate::domain::character::catalog::{HeadStyleCatalog, HeadStyleEntry};
use crate::domain::character::Gender;
use crate::infrastructure::assets::HierarchicalAssetManager;
use bevy::prelude::*;
use std::collections::HashMap;

/// Builder for discovering head styles from asset files
pub struct HeadStyleCatalogBuilder;

impl HeadStyleCatalogBuilder {
    /// Build catalog from asset manager
    pub fn build_from_asset_manager(manager: &HierarchicalAssetManager) -> HeadStyleCatalog {
        let mut catalog = HeadStyleCatalog::new();
        let mut temp_entries: HashMap<(Gender, u16), HeadStyleEntry> = HashMap::new();
        let mut palette_map: HashMap<(Gender, u16), Vec<u16>> = HashMap::new();

        info!("Starting head style catalog discovery from asset manager...");

        // Get all files from the asset manager
        let all_files = manager.list_files();

        // Filter for sprite and palette paths
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

        info!(
            "Scanning {} sprite files and {} palette files",
            all_sprite_paths.len(),
            all_palette_paths.len()
        );

        // Phase 1: Discover sprite/act files
        let mut sprites_found = 0;
        for path in &all_sprite_paths {
            let normalized_path = normalize_path(path);

            // Match sprite files
            if let Some(caps) = HAIR_SPRITE.captures(&normalized_path) {
                if let (Some(gender_str), Some(id_str)) = (caps.get(1), caps.get(2)) {
                    if let (Some(gender), Ok(id)) = (
                        parse_gender(gender_str.as_str()),
                        id_str.as_str().parse::<u16>(),
                    ) {
                        // Generate act path
                        let act_path = path.replace(".spr", ".act");

                        // Check if .act file exists in the loaded paths
                        let act_exists = all_sprite_paths.iter().any(|p| p == &act_path);

                        if act_exists {
                            // Add "ro://" prefix and normalize path separators to forward slashes
                            let sprite_path = format!("ro://{}", path.replace('\\', "/"));
                            let act_path_with_prefix = format!("ro://{}", act_path.replace('\\', "/"));

                            temp_entries.insert(
                                (gender, id),
                                HeadStyleEntry {
                                    id,
                                    gender,
                                    sprite_path,
                                    act_path: act_path_with_prefix,
                                    available_colors: vec![0], // Default color
                                },
                            );
                            sprites_found += 1;
                        } else {
                            debug!(
                                "Found sprite {} but missing corresponding .act file: {}",
                                path, act_path
                            );
                        }
                    }
                }
            }
        }

        info!("Phase 1 complete: Found {} valid hair sprites", sprites_found);

        // Phase 2: Discover palette files
        let mut palettes_found = 0;
        for path in &all_palette_paths {
            let normalized_path = normalize_path(path);

            if let Some(caps) = HAIR_PALETTE.captures(&normalized_path) {
                if let (Some(id_str), Some(gender_str), Some(color_str)) =
                    (caps.get(1), caps.get(2), caps.get(3))
                {
                    if let (Ok(id), Some(gender), Ok(color)) = (
                        id_str.as_str().parse::<u16>(),
                        parse_gender(gender_str.as_str()),
                        color_str.as_str().parse::<u16>(),
                    ) {
                        palette_map
                            .entry((gender, id))
                            .or_insert_with(Vec::new)
                            .push(color);
                        palettes_found += 1;
                    }
                }
            }
        }

        info!("Phase 2 complete: Found {} palette files", palettes_found);

        // Phase 3: Merge palette data into entries
        for ((gender, id), colors) in palette_map {
            if let Some(entry) = temp_entries.get_mut(&(gender, id)) {
                entry.available_colors.extend(colors);
                entry.available_colors.sort();
                entry.available_colors.dedup();
            } else {
                debug!(
                    "Found palettes for style {} ({:?}) but no corresponding sprite",
                    id, gender
                );
            }
        }

        // Phase 4: Add all entries to catalog
        for entry in temp_entries.into_values() {
            catalog.add(entry);
        }

        info!(
            "Head style catalog built: {} male styles, {} female styles, {} total",
            catalog.male_count(),
            catalog.female_count(),
            catalog.total_count()
        );

        // Log sample entries for verification
        if catalog.total_count() > 0 {
            info!("Sample entries:");
            for gender in [Gender::Male, Gender::Female] {
                if let Some(first) = catalog.get_all(gender).first() {
                    info!(
                        "  {:?} style {}: {} colors available",
                        gender,
                        first.id,
                        first.available_colors.len()
                    );
                }
            }
        } else {
            warn!("No head styles found! Check your asset paths and GRF files.");
        }

        catalog
    }
}

/// System to build catalog on startup
fn build_catalog_on_startup(
    mut commands: Commands,
    asset_manager: Option<Res<HierarchicalAssetManager>>,
    mut initialized: Local<bool>,
) {
    // Only run once
    if *initialized {
        return;
    }

    // Wait for asset manager to be ready
    let Some(asset_manager) = asset_manager else {
        return;
    };

    info!("Building head style catalog from asset manager...");
    let start = std::time::Instant::now();

    // Build head style catalog
    let head_catalog = HeadStyleCatalogBuilder::build_from_asset_manager(&asset_manager);
    commands.insert_resource(head_catalog);

    let elapsed = start.elapsed();
    info!("Asset catalog built in {:?}", elapsed);

    *initialized = true;
}

/// Plugin to register catalog building
pub struct AssetCatalogPlugin;

impl Plugin for AssetCatalogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, build_catalog_on_startup);
    }
}
