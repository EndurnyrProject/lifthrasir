//! The asset browser: an index of the `ro://` source's placeable assets and
//! a filterable left panel to pick one.

use super::{
    preview::{GRID_THUMBNAIL_SIZE, GridThumbnails, PreviewPlugin, PreviewState, preview_ui},
    ui::{EditorUiArea, EditorUiSet},
};
use crate::map::tools::{ToolMode, place::is_placeable};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_assets::{
    catalogs::EffectCatalog,
    source::{AssetSource, SharedCompositeAssetSource},
};

/// Placeable assets as `ro://` URIs, plus the effect catalog's `special` ids.
/// Every list is sorted.
#[derive(Resource, Default, Clone, Debug)]
pub struct AssetIndex {
    pub models: Vec<String>,
    pub ground_textures: Vec<String>,
    pub sounds: Vec<String>,
    pub effects: Vec<u32>,
}

impl AssetIndex {
    /// Indexes root-relative source paths (`models/x.glb`): props are
    /// `models/**/*.glb`, ground textures `ground/tex/**/*.ktx2`, sounds
    /// `data/wav/**/*.wav`. Paths are the source's normalized lowercase ones,
    /// so matching is case-sensitive.
    pub fn build(files: &[String], effects: impl IntoIterator<Item = u32>) -> AssetIndex {
        let pick = |prefix: &str, extension: &str| {
            let mut uris: Vec<String> = files
                .iter()
                .filter(|file| file.starts_with(prefix) && file.ends_with(extension))
                .map(|file| format!("ro://{file}"))
                .collect();
            uris.sort();
            uris
        };
        let mut effects: Vec<u32> = effects.into_iter().collect();
        effects.sort_unstable();
        AssetIndex {
            models: pick("models/", ".glb"),
            ground_textures: pick("ground/tex/", ".ktx2"),
            sounds: pick("data/wav/", ".wav"),
            effects,
        }
    }
}

/// One browser entry.
#[derive(Clone, Debug, PartialEq)]
pub enum AssetRef {
    Model(String),
    GroundTexture(String),
    Sound(String),
    Effect(u32),
}

/// The entry last clicked in the browser.
#[derive(Resource, Default)]
pub struct BrowserSelection(pub Option<AssetRef>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BrowserTab {
    #[default]
    Models,
    Ground,
    Sounds,
    Effects,
}

/// How models and ground textures are arranged in the browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AssetView {
    #[default]
    List,
    Grid,
}

#[derive(Resource, Default)]
struct BrowserPanel {
    tab: BrowserTab,
    filter: String,
    asset_view: AssetView,
}

/// Indexes the source at startup, adds effects once the catalog loads, and
/// draws the browser panel with the selection's preview.
pub struct BrowserPlugin;

impl Plugin for BrowserPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PreviewPlugin)
            .init_resource::<AssetIndex>()
            .init_resource::<BrowserSelection>()
            .init_resource::<BrowserPanel>()
            .add_systems(Startup, index_source)
            .add_systems(
                Update,
                index_effects.run_if(resource_exists_and_changed::<EffectCatalog>),
            )
            .add_systems(
                EguiPrimaryContextPass,
                browser_panel.in_set(EditorUiSet::LeftPanel),
            );
    }
}

fn build_index(source: &SharedCompositeAssetSource, catalog: Option<&EffectCatalog>) -> AssetIndex {
    let files = source
        .0
        .read()
        .expect("ro:// source lock poisoned")
        .list_files();
    AssetIndex::build(
        &files,
        catalog.into_iter().flat_map(EffectCatalog::special_ids),
    )
}

fn index_source(
    mut index: ResMut<AssetIndex>,
    source: Res<SharedCompositeAssetSource>,
    catalog: Option<Res<EffectCatalog>>,
) {
    *index = build_index(&source, catalog.as_deref());
}

fn index_effects(mut index: ResMut<AssetIndex>, catalog: Res<EffectCatalog>) {
    index.effects = catalog.special_ids().collect();
}

#[allow(clippy::too_many_arguments)]
fn browser_panel(
    mut contexts: EguiContexts,
    mut area: ResMut<EditorUiArea>,
    mut panel: ResMut<BrowserPanel>,
    mut index: ResMut<AssetIndex>,
    mut selection: ResMut<BrowserSelection>,
    source: Res<SharedCompositeAssetSource>,
    catalog: Option<Res<EffectCatalog>>,
    mut preview: ResMut<PreviewState>,
    mut thumbnails: ResMut<GridThumbnails>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
    mut mode: ResMut<ToolMode>,
) -> Result {
    let textures = preview.texture_ids(&mut contexts);
    let ctx = contexts.ctx_mut()?.clone();
    let panel = &mut *panel;
    thumbnails.begin_frame();
    let mut refresh = false;
    let mut next_selection = None;
    area.show(&ctx, "browser", |ui| {
        egui::Panel::left("browser")
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (tab, label) in [
                        (BrowserTab::Models, "Models"),
                        (BrowserTab::Ground, "Ground"),
                        (BrowserTab::Sounds, "Sounds"),
                        (BrowserTab::Effects, "Effects"),
                    ] {
                        ui.selectable_value(&mut panel.tab, tab, label);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Filter");
                    ui.text_edit_singleline(&mut panel.filter);
                    refresh = ui.button("Refresh").clicked();
                });
                if matches!(panel.tab, BrowserTab::Models | BrowserTab::Ground) {
                    ui.horizontal(|ui| {
                        ui.label("View");
                        ui.selectable_value(&mut panel.asset_view, AssetView::List, "List");
                        ui.selectable_value(&mut panel.asset_view, AssetView::Grid, "Grid");
                    });
                }
                ui.separator();
                egui::Panel::bottom("browser_preview").show(ui, |ui| {
                    if let Some(asset) = selection.0.as_ref().filter(|asset| is_placeable(asset))
                        && ui.button("Place").clicked()
                    {
                        *mode = ToolMode::Place(asset.clone());
                    }
                    preview_ui(ui, &selection, &mut preview, &textures);
                });
                if panel.tab == BrowserTab::Effects && catalog.is_none() {
                    ui.label("Effect catalog loading…");
                    return;
                }
                let entries = entries(&index, panel.tab, &panel.filter.to_lowercase());
                let view = matches!(panel.tab, BrowserTab::Models | BrowserTab::Ground)
                    .then_some(panel.asset_view)
                    .unwrap_or(AssetView::List);
                next_selection = match view {
                    AssetView::List => asset_list_ui(ui, &entries, &selection),
                    AssetView::Grid => asset_grid_ui(
                        ui,
                        &entries,
                        &selection,
                        &mut contexts,
                        &mut thumbnails,
                        &mut images,
                        &assets,
                    ),
                };
            });
    });
    if let Some(asset) = next_selection {
        selection.0 = Some(asset);
    }
    if refresh {
        *index = build_index(&source, catalog.as_deref());
    }
    Ok(())
}

fn asset_list_ui(
    ui: &mut egui::Ui,
    entries: &[(String, AssetRef)],
    selection: &BrowserSelection,
) -> Option<AssetRef> {
    let mut next_selection = None;
    let row_height = ui.text_style_height(&egui::TextStyle::Button);
    egui::ScrollArea::vertical().auto_shrink(false).show_rows(
        ui,
        row_height,
        entries.len(),
        |ui, rows| next_selection = select_entries(ui, &entries[rows], selection),
    );
    next_selection
}

/// Grid cells use fixed dimensions, so a long asset path cannot make
/// neighbouring cells shift.
fn asset_grid_ui(
    ui: &mut egui::Ui,
    entries: &[(String, AssetRef)],
    selection: &BrowserSelection,
    contexts: &mut EguiContexts,
    thumbnails: &mut GridThumbnails,
    images: &mut Assets<Image>,
    assets: &AssetServer,
) -> Option<AssetRef> {
    let mut next_selection = None;
    const CELL_WIDTH: f32 = 104.0;
    const CELL_HEIGHT: f32 = GRID_THUMBNAIL_SIZE as f32 + 28.0;
    let grid_width = ui.available_width();
    let columns = ((grid_width + ui.spacing().item_spacing.x)
        / (CELL_WIDTH + ui.spacing().item_spacing.x))
        .floor()
        .max(1.0) as usize;
    let rows = entries.len().div_ceil(columns);
    // Without a bounded viewport egui measures every row, which would request
    // a renderer for the entire model catalog.
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .max_width(grid_width)
        .show_rows(ui, CELL_HEIGHT, rows, |ui, rows| {
            for row in rows {
                let start = row * columns;
                let end = (start + columns).min(entries.len());
                ui.horizontal(|ui| {
                    for (label, asset) in &entries[start..end] {
                        let selected = selection.0.as_ref() == Some(asset);
                        let name = label.rsplit('/').next().unwrap_or(label);
                        let name = grid_label(name);
                        let texture = match asset {
                            AssetRef::Model(uri) => {
                                Some(thumbnails.model_texture_id(uri, contexts, images))
                            }
                            AssetRef::GroundTexture(uri) => {
                                Some(thumbnails.ground_texture_id(uri, contexts, assets))
                            }
                            _ => None,
                        };
                        let response = ui
                            .allocate_ui_with_layout(
                                egui::vec2(CELL_WIDTH, CELL_HEIGHT),
                                egui::Layout::top_down(egui::Align::Center),
                                |ui| {
                                    let thumbnail = texture.map(|texture| {
                                        ui.add(
                                            egui::Image::new(egui::load::SizedTexture::new(
                                                texture,
                                                egui::vec2(
                                                    GRID_THUMBNAIL_SIZE as f32,
                                                    GRID_THUMBNAIL_SIZE as f32,
                                                ),
                                            ))
                                            .sense(egui::Sense::click()),
                                        )
                                    });
                                    let name = ui.add_sized(
                                        egui::vec2(CELL_WIDTH, 22.0),
                                        egui::Button::selectable(selected, name),
                                    );
                                    match thumbnail {
                                        Some(thumbnail) => thumbnail.union(name),
                                        None => name,
                                    }
                                },
                            )
                            .inner
                            .on_hover_text(label);
                        if response.clicked() {
                            next_selection = Some(asset.clone());
                        }
                    }
                });
            }
        });
    next_selection
}

/// Fixed-width buttons must not use an unbounded filename as their desired
/// width: egui would grow the resizable browser panel to accommodate it.
fn grid_label(name: &str) -> String {
    const MAX_CHARS: usize = 14;
    let mut chars = name.chars();
    let abbreviated: String = chars.by_ref().take(MAX_CHARS).collect();
    if chars.next().is_some() {
        format!("{abbreviated}…")
    } else {
        abbreviated
    }
}

fn select_entries(
    ui: &mut egui::Ui,
    entries: &[(String, AssetRef)],
    selection: &BrowserSelection,
) -> Option<AssetRef> {
    let mut next_selection = None;
    for (label, asset) in entries {
        let selected = selection.0.as_ref() == Some(asset);
        if ui.selectable_label(selected, label).clicked() {
            next_selection = Some(asset.clone());
        }
    }
    next_selection
}

/// The tab's entries whose label contains `filter`, as (label, asset).
fn entries(index: &AssetIndex, tab: BrowserTab, filter: &str) -> Vec<(String, AssetRef)> {
    let uris = |uris: &[String], asset: fn(String) -> AssetRef| {
        uris.iter()
            .map(|uri| {
                (
                    uri.trim_start_matches("ro://").to_string(),
                    asset(uri.clone()),
                )
            })
            .filter(|(label, _)| label.contains(filter))
            .collect()
    };
    match tab {
        BrowserTab::Models => uris(&index.models, AssetRef::Model),
        BrowserTab::Ground => uris(&index.ground_textures, AssetRef::GroundTexture),
        BrowserTab::Sounds => uris(&index.sounds, AssetRef::Sound),
        BrowserTab::Effects => index
            .effects
            .iter()
            .map(|&id| (format!("effect {id}"), AssetRef::Effect(id)))
            .filter(|(label, _)| label.contains(filter))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_index_keeps_only_placeable_assets_sorted_as_uris() {
        let files: Vec<String> = [
            "models/prontera/tree.glb",
            "models/a.glb",
            "models/tex/bark.ktx2",
            "maps/prontera/prontera.glb",
            "ground/tex/grass.ktx2",
            "ground/tex/sub/dirt.ktx2",
            "ground/grass.ktx2",
            "data/wav/effect/water.wav",
            "data/wav/bgm.mp3",
            "wav/stray.wav",
        ]
        .map(String::from)
        .into();

        let index = AssetIndex::build(&files, [12, 3]);

        assert_eq!(
            index.models,
            ["ro://models/a.glb", "ro://models/prontera/tree.glb"]
        );
        assert_eq!(
            index.ground_textures,
            [
                "ro://ground/tex/grass.ktx2",
                "ro://ground/tex/sub/dirt.ktx2"
            ]
        );
        assert_eq!(index.sounds, ["ro://data/wav/effect/water.wav"]);
        assert_eq!(index.effects, [3, 12]);
    }
}
