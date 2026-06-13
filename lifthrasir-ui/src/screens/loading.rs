use bevy::prelude::*;
use bevy_extended_ui::html::HtmlSource;
use bevy_extended_ui::io::HtmlAsset;
use bevy_extended_ui::old::registry::UiRegistry;
use game_engine::core::state::GameState;

const LOADING_UI: &str = "loading";
/// `AssetServer` path, relative to `assets/` (NOT to `ExtendedUiConfiguration.assets_path`).
/// extended_ui resolves the `<link>` CSS hrefs inside the HTML relative to this file's own
/// location, so the stylesheets are referenced by bare name (`theme.css`) in `loading.html`.
const LOADING_HTML: &str = "ui/loading.html";

pub struct LoadingScreenPlugin;

impl Plugin for LoadingScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Loading), show_loading_screen);
        app.add_systems(OnExit(GameState::Loading), hide_loading_screen);
    }
}

#[allow(deprecated)]
fn show_loading_screen(mut registry: ResMut<UiRegistry>, asset_server: Res<AssetServer>) {
    let handle: Handle<HtmlAsset> = asset_server.load(LOADING_HTML);
    registry.add_and_use(LOADING_UI.into(), HtmlSource::from_handle(handle));
}

#[allow(deprecated)]
fn hide_loading_screen(mut registry: ResMut<UiRegistry>) {
    registry.remove(LOADING_UI);
}
