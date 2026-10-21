//! The asset browser: an index of the `ro://` source's placeable assets and
//! a filterable left panel to pick one.

use super::{
    preview::{PreviewPlugin, PreviewState, preview_ui},
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

#[derive(Resource, Default)]
struct BrowserPanel {
    tab: BrowserTab,
    filter: String,
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
    mut mode: ResMut<ToolMode>,
) -> Result {
    let textures = preview.texture_ids(&mut contexts);
    let ctx = contexts.ctx_mut()?;
    let panel = &mut *panel;
    let mut refresh = false;
    area.show(ctx, "browser", |ui| {
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
                let row_height = ui.text_style_height(&egui::TextStyle::Button);
                egui::ScrollArea::vertical().auto_shrink(false).show_rows(
                    ui,
                    row_height,
                    entries.len(),
                    |ui, rows| {
                        for (label, asset) in &entries[rows] {
                            let selected = selection.0.as_ref() == Some(asset);
                            if ui.selectable_label(selected, label).clicked() {
                                selection.0 = Some(asset.clone());
                            }
                        }
                    },
                );
            });
    });
    if refresh {
        *index = build_index(&source, catalog.as_deref());
    }
    Ok(())
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
