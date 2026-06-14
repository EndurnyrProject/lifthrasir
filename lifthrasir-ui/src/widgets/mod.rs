//! In-game HUD overlay (extended_ui).
//!
//! extended_ui's `UiRegistry` shows one screen at a time, so the chat box and the
//! character-info panel share a single `hud.html` screen; this module owns its
//! show/remove lifecycle on `GameState::InGame` and composes the two widget
//! sub-plugins that drive its elements by `CssID`.

use bevy::prelude::*;
use bevy_extended_ui::html::HtmlSource;
use bevy_extended_ui::io::HtmlAsset;
use bevy_extended_ui::old::registry::UiRegistry;
use game_engine::core::state::GameState;

pub mod character_info;
pub mod chat_box;

const HUD_UI: &str = "hud";
/// `AssetServer` path relative to `assets/`; the HTML's `<link>` hrefs resolve
/// relative to this file (so `theme.css` -> `ui/theme.css`).
const HUD_HTML: &str = "ui/hud.html";

pub struct InGameHudPlugin;

impl Plugin for InGameHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), show_hud);
        app.add_systems(OnExit(GameState::InGame), hide_hud);
        app.add_plugins((character_info::CharacterInfoPlugin, chat_box::ChatBoxPlugin));
    }
}

#[allow(deprecated)]
fn show_hud(mut registry: ResMut<UiRegistry>, asset_server: Res<AssetServer>) {
    let handle: Handle<HtmlAsset> = asset_server.load(HUD_HTML);
    registry.add_and_use(HUD_UI.into(), HtmlSource::from_handle(handle));
}

#[allow(deprecated)]
fn hide_hud(mut registry: ResMut<UiRegistry>) {
    registry.remove(HUD_UI);
}
