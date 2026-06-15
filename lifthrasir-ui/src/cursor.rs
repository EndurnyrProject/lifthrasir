//! Native RO cursor.
//!
//! The Ragnarok cursors are static single-frame PNGs (one per [`CursorType`]), so
//! Bevy 0.18's built-in `CursorIcon::Custom` does the whole job — no sprite-sheet
//! crate, no hand-drawn UI node, no hiding the OS cursor (the custom image *is* the
//! OS cursor). The engine drives [`CurrentCursorType`] from terrain/hover state; we
//! mirror it onto the primary window whenever it changes.

use bevy::prelude::*;
use bevy::window::{CursorIcon, CustomCursor, CustomCursorImage, PrimaryWindow};
use game_engine::domain::input::{CurrentCursorType, CursorType};

/// `AssetServer` path (relative to `assets/`) holding the extracted cursor PNGs.
const CURSOR_DIR: &str = "data/textures/ui/cursors";

pub struct NativeCursorPlugin;

impl Plugin for NativeCursorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AppliedCursor>();
        app.add_systems(Startup, load_cursor_textures);
        app.add_systems(Update, apply_cursor);
    }
}

#[derive(Resource)]
struct CursorTextures {
    default: Handle<Image>,
    add: Handle<Image>,
    attack: Handle<Image>,
    impossible: Handle<Image>,
    talk: Handle<Image>,
}

impl CursorTextures {
    fn handle(&self, cursor: CursorType) -> Handle<Image> {
        match cursor {
            CursorType::Default => self.default.clone(),
            CursorType::Add => self.add.clone(),
            CursorType::Attack => self.attack.clone(),
            CursorType::Impossible => self.impossible.clone(),
            CursorType::Talk => self.talk.clone(),
        }
    }
}

/// Last cursor type pushed to the window, so we only re-insert on a real change.
#[derive(Resource, Default)]
struct AppliedCursor(Option<CursorType>);

/// Click point within each cursor image, in pixels (from the original RO cursors).
fn hotspot(cursor: CursorType) -> (u16, u16) {
    match cursor {
        CursorType::Attack => (10, 5),
        CursorType::Default | CursorType::Add | CursorType::Impossible | CursorType::Talk => {
            (17, 17)
        }
    }
}

fn load_cursor_textures(mut commands: Commands, asset_server: Res<AssetServer>) {
    let load = |name: &str| asset_server.load(format!("{CURSOR_DIR}/{name}"));
    commands.insert_resource(CursorTextures {
        default: load("cursor_default.png"),
        add: load("cursor_add.png"),
        attack: load("cursor_attack.png"),
        impossible: load("cursor_impossible.png"),
        talk: load("cursor_talk.png"),
    });
}

/// Pushes the current cursor image onto the primary window once its PNG has
/// loaded. Gating on load avoids winit's per-frame "image not loaded yet" warning,
/// and the `AppliedCursor` guard re-runs the insert only when the type changes.
fn apply_cursor(
    current: Res<CurrentCursorType>,
    textures: Res<CursorTextures>,
    images: Res<Assets<Image>>,
    mut applied: ResMut<AppliedCursor>,
    mut commands: Commands,
    window: Query<Entity, With<PrimaryWindow>>,
) {
    let desired = current.get();
    if applied.0 == Some(desired) {
        return;
    }
    let handle = textures.handle(desired);
    if images.get(&handle).is_none() {
        return;
    }
    let Ok(window) = window.single() else {
        return;
    };
    commands
        .entity(window)
        .insert(CursorIcon::Custom(CustomCursor::Image(CustomCursorImage {
            handle,
            texture_atlas: None,
            flip_x: false,
            flip_y: false,
            rect: None,
            hotspot: hotspot(desired),
        })));
    applied.0 = Some(desired);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_hotspot_differs_from_centered_cursors() {
        assert_eq!(hotspot(CursorType::Attack), (10, 5));
        assert_eq!(hotspot(CursorType::Default), (17, 17));
        assert_eq!(hotspot(CursorType::Add), (17, 17));
        assert_eq!(hotspot(CursorType::Impossible), (17, 17));
        assert_eq!(hotspot(CursorType::Talk), (17, 17));
    }
}
