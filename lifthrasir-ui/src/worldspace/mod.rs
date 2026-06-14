//! World-anchored overlays: hover nameplates and floating damage numbers.
//!
//! These are screen-projected `bevy_ui` text nodes (not `bevy_lunex` worldspace
//! UI): each frame an anchored node's `left`/`top` is set from
//! `Camera::world_to_viewport(entity_position)`. This keeps the classic RO
//! always-on-top look, adds no dependency, and needs no changes to the in-game
//! `Camera3d` — the lighter, lower-risk path chosen over lunex.

use bevy::prelude::*;
use bevy::text::Font;

use crate::theme;

pub mod damage_numbers;
pub mod nameplates;

/// Shared font handle for nameplates and damage numbers, loaded once at startup.
#[derive(Resource)]
pub struct WorldspaceFont(pub Handle<Font>);

pub struct WorldspaceUiPlugin;

impl Plugin for WorldspaceUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_font);
        app.add_plugins((
            nameplates::NameplatePlugin,
            damage_numbers::DamageNumberPlugin,
        ));
    }
}

fn load_font(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(WorldspaceFont(asset_server.load(theme::FONT_BODY)));
}
